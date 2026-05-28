//! Topological dependency resolution for engine module initialization order.

use crate::error::{ModuleError, ModuleResult};
use std::any::TypeId;
use std::collections::HashMap;
use theos_shared::EngineModule;

/// Computes a safe initialization order for a set of engine modules.
///
/// Uses Kahn's topological sort to guarantee that every module is initialized
/// before any module that depends on it. Missing dependencies, circular
/// dependencies, and version mismatches are detected eagerly and returned as
/// errors before any `init()` call is made.
pub struct DependencyResolver;

impl DependencyResolver {
    /// Returns indices into `modules` in a valid initialization order.
    ///
    /// For every position `i` in the returned `Vec<usize>`, all dependencies
    /// of `modules[result[i]]` appear at positions `< i`. The resolved order
    /// is logged at `DEBUG` level as a comma-separated list of module names.
    ///
    /// # Errors
    /// - [`ModuleError::DependencyMissing`] — a module declares a dependency
    ///   on a [`TypeId`] not present in the `modules` slice.
    /// - [`ModuleError::VersionMismatch`] — a dependency is present but its
    ///   version does not satisfy the declared [`VersionReq`] constraint.
    /// - [`ModuleError::CircularDependency`] — the dependency graph contains a
    ///   cycle (e.g. A → B → A).
    ///
    /// [`TypeId`]: TypeId
    /// [`VersionReq`]: semver::VersionReq
    pub fn sort(modules: &[Box<dyn EngineModule>]) -> ModuleResult<Vec<usize>> {
        struct ModuleInfo {
            index: usize,
            dependents: Vec<TypeId>,
            unresolved_deps: usize,
        }
        let mut module_map: HashMap<TypeId, ModuleInfo> = HashMap::new();
        for (index, module) in modules.iter().enumerate() {
            module_map.insert(
                module.module_type_id(),
                ModuleInfo {
                    index,
                    dependents: Vec::new(),
                    unresolved_deps: module.dependencies().len(),
                },
            );
        }
        for module in modules {
            let dependencies = module.dependencies();
            for dep in dependencies {
                match module_map.get_mut(&dep.module) {
                    Some(module_info) => {
                        let version = modules[module_info.index].version();
                        if !dep.constraint.matches(&version) {
                            return Err(ModuleError::VersionMismatch {
                                dependent: module.name(),
                                dependency: modules[module_info.index].name(),
                                required: dep.constraint.clone(),
                                found: version,
                            });
                        }
                        module_info.dependents.push(module.module_type_id());
                    }
                    None => {
                        tracing::error!(
                            "Module {} declares unknown dependency: {:?}",
                            module.name(),
                            dep.module
                        );
                        return Err(ModuleError::DependencyMissing {
                            module: module.name().to_owned(),
                            dependency: dep.name.to_owned(),
                        });
                    }
                }
            }
        }
        let mut sorted: Vec<usize> = Vec::new();
        let mut ready_queue: Vec<TypeId> = module_map
            .keys()
            .filter(|id| module_map.get(id).unwrap().unresolved_deps == 0)
            .cloned()
            .collect();
        while let Some(module) = ready_queue.pop() {
            let module_info = module_map.get(&module).unwrap();
            sorted.push(module_info.index);
            for dependent in &module_info.dependents.clone() {
                let dependent_info = module_map.get_mut(dependent).unwrap();
                dependent_info.unresolved_deps -= 1;
                if dependent_info.unresolved_deps == 0 {
                    ready_queue.push(*dependent);
                }
            }
        }
        if sorted.len() != modules.len() {
            return Err(ModuleError::CircularDependency {
                cycle: module_map
                    .iter()
                    .filter(|(_, info)| info.unresolved_deps > 0)
                    .map(|(_, info)| modules[info.index].name())
                    .collect::<Vec<&str>>()
                    .join(" -> "),
            });
        }

        tracing::debug!(
            "Module init order: {}",
            sorted
                .iter()
                .map(|&i| modules[i].name())
                .collect::<Vec<_>>()
                .join(", ")
        );

        Ok(sorted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::VersionReq;
    use std::any::TypeId;
    use std::marker::PhantomData;
    use std::sync::LazyLock;
    use theos_shared::KernelContext;
    use theos_shared::ModuleDependency;

    // -----------------------------------------------------------------------
    // TestModule<Tag> — minimal stub whose TypeId is determined by the Tag
    // type parameter, allowing distinct identity without a new struct per test.
    // -----------------------------------------------------------------------

    struct TestModule<Tag: 'static> {
        name: &'static str,
        deps: &'static [ModuleDependency],
        _tag: PhantomData<Tag>,
    }

    impl<Tag: Send + Sync + 'static> EngineModule for TestModule<Tag> {
        fn name(&self) -> &'static str {
            self.name
        }
        fn dependencies(&self) -> &[ModuleDependency] {
            self.deps
        }
        fn init(
            &mut self,
            _ctx: &mut KernelContext,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn module_type_id(&self) -> TypeId {
            TypeId::of::<TestModule<Tag>>()
        }
    }

    // One tag type per logically distinct module identity.
    struct TagA;
    struct TagB;
    struct TagC;
    struct TagX;
    struct TagY;
    struct TagZ;
    struct TagIndep1;
    struct TagIndep2;
    struct TagPhysics; // represents an unregistered module in AC2

    // Convenience: construct a boxed TestModule with no deps.
    fn module<Tag: Send + Sync + 'static>(name: &'static str) -> Box<dyn EngineModule> {
        Box::new(TestModule::<Tag> {
            name,
            deps: &[],
            _tag: PhantomData,
        })
    }

    // Convenience: construct a boxed TestModule with a static dep slice.
    fn module_with_deps<Tag: Send + Sync + 'static>(
        name: &'static str,
        deps: &'static [ModuleDependency],
    ) -> Box<dyn EngineModule> {
        Box::new(TestModule::<Tag> {
            name,
            deps,
            _tag: PhantomData,
        })
    }

    // -----------------------------------------------------------------------
    // AC1: Linear chain A → B → C resolves in dependency order.
    // -----------------------------------------------------------------------

    #[test]
    fn linear_chain_resolves_in_dependency_order() {
        static B_DEPS: LazyLock<Vec<ModuleDependency>> =
            LazyLock::new(|| vec![ModuleDependency::any("A", TypeId::of::<TestModule<TagA>>())]);
        static C_DEPS: LazyLock<Vec<ModuleDependency>> =
            LazyLock::new(|| vec![ModuleDependency::any("B", TypeId::of::<TestModule<TagB>>())]);

        let modules = vec![
            module::<TagA>("A"),
            module_with_deps::<TagB>("B", &B_DEPS),
            module_with_deps::<TagC>("C", &C_DEPS),
        ];

        let result = DependencyResolver::sort(&modules).unwrap();
        assert_eq!(result.len(), 3);

        let pos = |name: &str| {
            result
                .iter()
                .position(|&i| modules[i].name() == name)
                .unwrap()
        };
        assert!(pos("A") < pos("B"), "A must appear before B");
        assert!(pos("B") < pos("C"), "B must appear before C");
    }

    // -----------------------------------------------------------------------
    // AC2: Missing dependency returns DependencyMissing with correct fields.
    // -----------------------------------------------------------------------

    #[test]
    fn missing_dependency_returns_dependency_missing_error() {
        static B_DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
            vec![ModuleDependency::any(
                "PhysicsModule",
                TypeId::of::<TestModule<TagPhysics>>(), // not registered below
            )]
        });

        let modules = vec![module_with_deps::<TagB>("B", &B_DEPS)];

        assert!(matches!(
            DependencyResolver::sort(&modules),
            Err(ModuleError::DependencyMissing { module, dependency })
                if module == "B" && dependency == "PhysicsModule"
        ));
    }

    // -----------------------------------------------------------------------
    // AC3: 2-node cycle returns CircularDependency naming both modules.
    // -----------------------------------------------------------------------

    #[test]
    fn two_node_cycle_returns_circular_dependency_error() {
        static A_DEPS: LazyLock<Vec<ModuleDependency>> =
            LazyLock::new(|| vec![ModuleDependency::any("B", TypeId::of::<TestModule<TagB>>())]);
        static B_DEPS: LazyLock<Vec<ModuleDependency>> =
            LazyLock::new(|| vec![ModuleDependency::any("A", TypeId::of::<TestModule<TagA>>())]);

        let modules = vec![
            module_with_deps::<TagA>("A", &A_DEPS),
            module_with_deps::<TagB>("B", &B_DEPS),
        ];

        match DependencyResolver::sort(&modules) {
            Err(ModuleError::CircularDependency { cycle }) => {
                assert!(cycle.contains("A"), "cycle string must mention A: {cycle}");
                assert!(cycle.contains("B"), "cycle string must mention B: {cycle}");
            }
            other => panic!("expected CircularDependency, got: {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // AC4: 5-module graph — every dep appears strictly before its dependent.
    // -----------------------------------------------------------------------

    #[test]
    fn five_module_mixed_graph_satisfies_all_dependencies() {
        static Y_DEPS: LazyLock<Vec<ModuleDependency>> =
            LazyLock::new(|| vec![ModuleDependency::any("X", TypeId::of::<TestModule<TagX>>())]);
        static Z_DEPS: LazyLock<Vec<ModuleDependency>> =
            LazyLock::new(|| vec![ModuleDependency::any("Y", TypeId::of::<TestModule<TagY>>())]);

        let modules = vec![
            module::<TagIndep1>("Indep1"),
            module::<TagIndep2>("Indep2"),
            module::<TagX>("X"),
            module_with_deps::<TagY>("Y", &Y_DEPS),
            module_with_deps::<TagZ>("Z", &Z_DEPS),
        ];

        let result = DependencyResolver::sort(&modules).unwrap();
        assert_eq!(result.len(), 5);

        // For every module at position i, all its declared deps must be at position < i.
        for (pos, &idx) in result.iter().enumerate() {
            for dep in modules[idx].dependencies() {
                let dep_pos = result
                    .iter()
                    .position(|&i| modules[i].name() == dep.name)
                    .unwrap();
                assert!(
                    dep_pos < pos,
                    "'{}' (pos {}) must appear before '{}' (pos {})",
                    dep.name,
                    dep_pos,
                    modules[idx].name(),
                    pos
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Extra: empty input returns an empty order without error.
    // -----------------------------------------------------------------------

    #[test]
    fn empty_module_list_returns_empty_order() {
        let modules: Vec<Box<dyn EngineModule>> = vec![];
        assert_eq!(
            DependencyResolver::sort(&modules).unwrap(),
            Vec::<usize>::new()
        );
    }

    // -----------------------------------------------------------------------
    // Extra: single module with no deps resolves immediately.
    // -----------------------------------------------------------------------

    #[test]
    fn single_module_no_deps_resolves_to_itself() {
        let modules = vec![module::<TagA>("A")];
        assert_eq!(DependencyResolver::sort(&modules).unwrap(), vec![0]);
    }

    // -----------------------------------------------------------------------
    // Extra: version mismatch returns VersionMismatch naming both modules.
    // -----------------------------------------------------------------------

    #[test]
    fn version_mismatch_returns_version_mismatch_error() {
        // A provides 1.0.0 (default); B requires >=2.0.0 — mismatch.
        static B_DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
            vec![ModuleDependency::new(
                "A",
                TypeId::of::<TestModule<TagA>>(),
                ">=2.0.0".parse::<VersionReq>().unwrap(),
            )]
        });

        let modules = vec![module::<TagA>("A"), module_with_deps::<TagB>("B", &B_DEPS)];

        assert!(matches!(
            DependencyResolver::sort(&modules),
            Err(ModuleError::VersionMismatch {
                dependent: "B",
                dependency: "A",
                ..
            })
        ));
    }

    // -----------------------------------------------------------------------
    // Extra: longer cycle (3-node) is also detected.
    // -----------------------------------------------------------------------

    #[test]
    fn three_node_cycle_returns_circular_dependency_error() {
        static A_DEPS: LazyLock<Vec<ModuleDependency>> =
            LazyLock::new(|| vec![ModuleDependency::any("C", TypeId::of::<TestModule<TagC>>())]);
        static B_DEPS: LazyLock<Vec<ModuleDependency>> =
            LazyLock::new(|| vec![ModuleDependency::any("A", TypeId::of::<TestModule<TagA>>())]);
        static C_DEPS: LazyLock<Vec<ModuleDependency>> =
            LazyLock::new(|| vec![ModuleDependency::any("B", TypeId::of::<TestModule<TagB>>())]);

        let modules = vec![
            module_with_deps::<TagA>("A", &A_DEPS),
            module_with_deps::<TagB>("B", &B_DEPS),
            module_with_deps::<TagC>("C", &C_DEPS),
        ];

        match DependencyResolver::sort(&modules) {
            Err(ModuleError::CircularDependency { cycle }) => {
                assert!(cycle.contains("A"), "cycle must mention A: {cycle}");
                assert!(cycle.contains("B"), "cycle must mention B: {cycle}");
                assert!(cycle.contains("C"), "cycle must mention C: {cycle}");
            }
            other => panic!("expected CircularDependency, got: {other:?}"),
        }
    }
}
