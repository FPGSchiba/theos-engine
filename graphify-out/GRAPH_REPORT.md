# Graph Report - theos-engine  (2026-06-05)

## Corpus Check
- 48 files · ~30,757 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 898 nodes · 1697 edges · 57 communities (40 shown, 17 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 33 edges (avg confidence: 0.79)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `0f5bcf35`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- [[_COMMUNITY_Module Lifecycle & Kernel Fixtures|Module Lifecycle & Kernel Fixtures]]
- [[_COMMUNITY_Core Shared Abstractions|Core Shared Abstractions]]
- [[_COMMUNITY_Crossbeam EventBus Implementation|Crossbeam EventBus Implementation]]
- [[_COMMUNITY_Dependency Resolution Engine|Dependency Resolution Engine]]
- [[_COMMUNITY_Kernel Init & Module Wiring|Kernel Init & Module Wiring]]
- [[_COMMUNITY_Matrix Math & Benchmarks|Matrix Math & Benchmarks]]
- [[_COMMUNITY_Error Display & Formatting|Error Display & Formatting]]
- [[_COMMUNITY_Quaternion Arithmetic|Quaternion Arithmetic]]
- [[_COMMUNITY_AABB Spatial Queries|AABB Spatial Queries]]
- [[_COMMUNITY_Error Hierarchy & Conversions|Error Hierarchy & Conversions]]
- [[_COMMUNITY_Vector Math & Interop Tests|Vector Math & Interop Tests]]
- [[_COMMUNITY_Architecture Concepts & Benchmarks|Architecture Concepts & Benchmarks]]
- [[_COMMUNITY_CI Pipeline & Dependency Automation|CI Pipeline & Dependency Automation]]
- [[_COMMUNITY_Ray-Plane Intersection|Ray-Plane Intersection]]
- [[_COMMUNITY_SIMD & Interop Bridge Patterns|SIMD & Interop Bridge Patterns]]
- [[_COMMUNITY_Service Registry & Lookup|Service Registry & Lookup]]
- [[_COMMUNITY_Community 16|Community 16]]
- [[_COMMUNITY_Vec3 & Quaternion Ops|Vec3 & Quaternion Ops]]
- [[_COMMUNITY_Vector Arithmetic Ops|Vector Arithmetic Ops]]
- [[_COMMUNITY_Structured Logging Formatter|Structured Logging Formatter]]
- [[_COMMUNITY_Ray Traversal & BVH|Ray Traversal & BVH]]
- [[_COMMUNITY_Frame Timing Benchmarks|Frame Timing Benchmarks]]
- [[_COMMUNITY_SIMD Arithmetic Tests|SIMD Arithmetic Tests]]
- [[_COMMUNITY_Module Lifecycle Protocol|Module Lifecycle Protocol]]
- [[_COMMUNITY_Entity Identity|Entity Identity]]
- [[_COMMUNITY_Launcher & Log Filtering|Launcher & Log Filtering]]
- [[_COMMUNITY_Log Output Routing|Log Output Routing]]
- [[_COMMUNITY_Tracing Field Visitor|Tracing Field Visitor]]
- [[_COMMUNITY_Workspace Architecture Principles|Workspace Architecture Principles]]
- [[_COMMUNITY_EventBus SendDrain Benchmarks|EventBus Send/Drain Benchmarks]]
- [[_COMMUNITY_Engine Entry Points|Engine Entry Points]]
- [[_COMMUNITY_Service Discovery Test Fixtures|Service Discovery Test Fixtures]]
- [[_COMMUNITY_ECS Placeholder|ECS Placeholder]]
- [[_COMMUNITY_Physics Event System|Physics Event System]]
- [[_COMMUNITY_AI Subsystem Placeholder|AI Subsystem Placeholder]]
- [[_COMMUNITY_Render Subsystem Placeholder|Render Subsystem Placeholder]]
- [[_COMMUNITY_Assets Subsystem Placeholder|Assets Subsystem Placeholder]]
- [[_COMMUNITY_Plugin Loader Placeholder|Plugin Loader Placeholder]]
- [[_COMMUNITY_Physics Subsystem Placeholder|Physics Subsystem Placeholder]]
- [[_COMMUNITY_Plugin API Placeholder|Plugin API Placeholder]]
- [[_COMMUNITY_Scripting Subsystem Placeholder|Scripting Subsystem Placeholder]]
- [[_COMMUNITY_Scripting Lib Placeholder|Scripting Lib Placeholder]]
- [[_COMMUNITY_Game Main Stub|Game Main Stub]]
- [[_COMMUNITY_Audio Subsystem Placeholder|Audio Subsystem Placeholder]]
- [[_COMMUNITY_AI Mentor Agent Protocol|AI Mentor Agent Protocol]]
- [[_COMMUNITY_Community 51|Community 51]]
- [[_COMMUNITY_Community 52|Community 52]]
- [[_COMMUNITY_Community 53|Community 53]]
- [[_COMMUNITY_Community 54|Community 54]]
- [[_COMMUNITY_Community 56|Community 56]]

## God Nodes (most connected - your core abstractions)
1. `Vec3` - 59 edges
2. `Vec4` - 45 edges
3. `Self` - 42 edges
4. `Vec2` - 38 edges
5. `Aabb` - 31 edges
6. `TheosKernel` - 28 edges
7. `KernelContext` - 18 edges
8. `Quat` - 16 edges
9. `TimingModule` - 16 edges
10. `Mat4` - 14 edges

## Surprising Connections (you probably didn't know these)
- `Type-Erased EventBus via ChannelHandle adapter` --semantically_similar_to--> `Service Locator encapsulated in KernelContext`  [INFERRED] [semantically similar]
  theos-core/src/event.rs → plans/THEOS-86.md
- `PhysicsEvent` --semantically_similar_to--> `GameEvent`  [INFERRED] [semantically similar]
  theos-physics/src/event.rs → theos-core/src/event.rs
- `THEOS-86 Microkernel Bootstrap Plan` --references--> `TheosKernel`  [EXTRACTED]
  plans/THEOS-86.md → theos-core/src/kernel.rs
- `THEOS-86 Microkernel Bootstrap Plan` --references--> `CrossbeamEventBus`  [EXTRACTED]
  plans/THEOS-86.md → theos-core/src/event.rs
- `THEOS-86 Microkernel Bootstrap Plan` --rationale_for--> `Fixed-Step Game Loop (60 Hz sleep+spin hybrid)`  [EXTRACTED]
  plans/THEOS-86.md → theos-core/src/kernel.rs

## Hyperedges (group relationships)
- **SIMD-aligned math types (Vec2, Vec3, Vec4) forming the GPU-ready math foundation** — src_vec_vec2, src_vec_vec3, src_vec_vec4, concept_simd_alignment_16byte [EXTRACTED 0.95]
- **Kernel init pattern: KernelContext + ServiceLocator + EventBus shared during module init** — src_kernel_context_kernelcontext, src_service_locator_servicelocator, src_event_bus_eventbus, src_module_enginemodule [EXTRACTED 0.95]
- **EventBus three-trait split: EventBus (dyn), EventBusExt (generic blanket), EventBusSetup (setup-only)** — src_event_bus_eventbus, src_event_bus_eventbusext, src_event_bus_eventbussetup [EXTRACTED 0.95]
- **Kernel Lifecycle: Register → DependencyResolver sort → init → fixed-step loop → shutdown** — src_lib_theoskernel, src_lib_dependencyresolver, concept_fixed_step_loop [EXTRACTED 1.00]
- **Type-Erased EventBus: CrossbeamEventBus stores ChannelHandle<TypedChannel<E>> keyed by TypeId** — src_lib_crossbeameventbus, src_lib_channelhandle, src_lib_typedchannel [EXTRACTED 1.00]
- **Cross-Module Service Discovery via KernelContext: ProviderModule registers, ConsumerModule retrieves** — kernel_integration_providermodule, kernel_integration_consumermodule, kernel_integration_greetingservice [EXTRACTED 1.00]
- **CI Quality Gate Pipeline (Check, Lint, Test)** — workflows_check-lint_check_lint_workflow, workflows_test_test_workflow, workflows_check-lint_clippy_unsafe_policy_job [INFERRED 0.85]
- **Unsafe Code Audit Triad (Clippy, Forbid, Geiger)** — workflows_check-lint_clippy_unsafe_policy_job, workflows_check-lint_forbid_unsafe_verification, workflows_check-lint_cargo_geiger [EXTRACTED 0.95]
- **Release Artifact Production (Launcher, Game, Editor)** — workflows_release-packaging_theos_launcher_artifact, workflows_release-packaging_theos_game_artifact, workflows_release-packaging_theos_editor_artifact [EXTRACTED 1.00]

## Communities (57 total, 17 thin omitted)

### Community 0 - "Module Lifecycle & Kernel Fixtures"
Cohesion: 0.07
Nodes (38): AtomicU32, Instant, Mutex, ConsumerModule, CountingModule, CountingModule<Tag>, GreetingService, kernel_runs_init_tick_shutdown_lifecycle() (+30 more)

### Community 1 - "Core Shared Abstractions"
Cohesion: 0.12
Nodes (27): KernelContext is a concrete struct (not a trait) for type-safe service access, KernelContextResult, ServiceLocator, event_bus_hands_back_the_same_shared_handle(), get_unregistered_service_returns_service_not_found(), KernelContext, make_context(), register_overwrites_previous_service_of_same_type() (+19 more)

### Community 2 - "Crossbeam EventBus Implementation"
Cohesion: 0.09
Nodes (21): EventBusSetup, ChannelHandle, CrossbeamEventBus, GameEvent, Test2Event, TestEvent, TypedChannel<E>, Any (+13 more)

### Community 3 - "Dependency Resolution Engine"
Cohesion: 0.08
Nodes (31): ModuleResult, DependencyResolver, five_module_mixed_graph_satisfies_all_dependencies(), linear_chain_resolves_in_dependency_order(), module(), module_with_deps(), TagA, TagB (+23 more)

### Community 4 - "Kernel Init & Module Wiring"
Cohesion: 0.06
Nodes (55): CrossbeamEventBus, Default, KernelResult, calling_init_twice_returns_already_initialized(), CallLog, default_constructs_kernel_with_no_modules_and_uninitialised_flag(), FailingInitModule, frame_timer_clamps_huge_delta_times_to_max_limit() (+47 more)

### Community 5 - "Matrix Math & Benchmarks"
Cohesion: 0.10
Nodes (27): bench_mat3_mul(), bench_mat4_mul(), bench_vec3_cross(), bench_vec3_dot(), glam::Mat3, glam::Mat4, Mat3, Mat4 (+19 more)

### Community 6 - "Error Display & Formatting"
Cohesion: 0.12
Nodes (16): EventBusError, kernel_context_wraps_service_locator_error_and_exposes_source(), KernelContextError, ServiceLocatorError, ServiceLocatorOperation, TheosError enum, Display, E (+8 more)

### Community 7 - "Quaternion Arithmetic"
Cohesion: 0.15
Nodes (23): assert_quat_eq(), assert_vec3_eq(), Quat, rot90_z(), test_length_known_value(), test_mul_assign(), test_mul_identity_left(), test_mul_identity_right() (+15 more)

### Community 8 - "AABB Spatial Queries"
Cohesion: 0.16
Nodes (25): Aabb, assert_vec_eq(), test_contains_point_interior(), test_contains_point_on_surface_is_inside(), test_contains_point_outside(), test_expand_to_contain_point_inside_is_noop(), test_expand_to_contain_point_outside(), test_merge_is_symmetric() (+17 more)

### Community 9 - "Error Hierarchy & Conversions"
Cohesion: 0.11
Nodes (15): EventBusError, ServiceLocatorError, KernelError, module_error_converts_to_theos_error_wrapped(), ModuleError, Display, Error, FmtResult (+7 more)

### Community 10 - "Vector Math & Interop Tests"
Cohesion: 0.16
Nodes (21): test_glam_vec2_roundtrip(), test_glam_vec3_and_vec3a_produce_same_components(), test_glam_vec3_roundtrip(), test_glam_vec3a_roundtrip(), test_glam_vec4_roundtrip(), test_vec2_constructors_and_dot(), test_vec2_dot_antiparallel_is_negative(), test_vec2_dot_orthogonal_is_zero() (+13 more)

### Community 11 - "Architecture Concepts & Benchmarks"
Cohesion: 0.11
Nodes (22): Fixed-Step Game Loop (60 Hz sleep+spin hybrid), Kahn's Topological Sort for Module Init Order, Service Locator encapsulated in KernelContext, Type-Erased EventBus via ChannelHandle adapter, allocation_check_typed_event_bus (alloc test), bench_send_then_drain_typed (Criterion benchmark), bench_tick_dispatch (Criterion benchmark), NoOpModule (bench) (+14 more)

### Community 12 - "CI Pipeline & Dependency Automation"
Cohesion: 0.09
Nodes (23): Cargo Plugins Workspace Dependency Updates (theos-plugin-deps), Cargo Workspace Dependency Updates (theos-engine-deps), Dependabot Configuration, GitHub Actions Dependency Updates, cargo-geiger Unsafe Audit Tool, Cargo Check Job, Check and Lint Workflow, Clippy and Unsafe Policy Job (+15 more)

### Community 13 - "Ray-Plane Intersection"
Cohesion: 0.22
Nodes (17): Ray, assert_vec3_eq(), Plane, test_intersects_ray_from_above_hits_origin(), test_intersects_ray_from_below_hits_plane(), test_intersects_ray_offset_origin(), test_intersects_ray_parallel_returns_none(), test_new_normalizes_normal() (+9 more)

### Community 14 - "SIMD & Interop Bridge Patterns"
Cohesion: 0.12
Nodes (9): Add, glam interoperability bridge pattern, mint interoperability bridge pattern, 16-byte SIMD alignment for GPU/SIMD compatibility, f32x4, glam::Vec2, mint::Vector2<f32>, Vec2 (+1 more)

### Community 15 - "Service Registry & Lookup"
Cohesion: 0.04
Nodes (44): 1.1 Define `EngineModule` Trait (in `theos-shared`), 1.2 Define `EventBus` Trait (in `theos-shared`), 1.3 Create `ServiceLocator` Struct (in `theos-shared`), 1.4 Stub Out `TheosKernel` Struct (in `theos-core`), 1.5 Define `AiWorkerHandle` Struct (in `theos-core`), 1.6 Define `AiCommandQueue` Struct (in `theos-core`), 1.7 Design `ModuleRegistry` (Logical Component), 2.1 Logging Initialization (+36 more)

### Community 16 - "Community 16"
Cohesion: 0.16
Nodes (15): Service Locator pattern for engine subsystem registry, RwLock, ServiceLocatorResult, register_and_get_service(), service_locator_is_cloneable_and_shares_registry(), ServiceLocator, TestService, Any (+7 more)

### Community 17 - "Vec3 & Quaternion Ops"
Cohesion: 0.15
Nodes (13): DivAssign, Mat3 * Vec3, Quat * Vec3 (Rodrigues rotation), f32, glam::Vec3, glam::Vec3A, mint::Vector3<f32>, Vec3 (+5 more)

### Community 19 - "Structured Logging Formatter"
Cohesion: 0.16
Nodes (13): FmtContext, FormatEvent, N, Rfc3339, S, FieldVisitor, init(), TheosFormatter (+5 more)

### Community 20 - "Ray Traversal & BVH"
Cohesion: 0.30
Nodes (13): BVH / ray-traversal usage pattern for Aabb and Ray, assert_vec3_eq(), Ray, test_at_known_t_value(), test_at_negative_t_goes_behind_origin(), test_at_one_returns_origin_plus_direction(), test_at_zero_returns_origin(), test_new_accepts_axis_aligned_directions() (+5 more)

### Community 21 - "Frame Timing Benchmarks"
Cohesion: 0.15
Nodes (11): bench_tick_dispatch(), NoOpModule, Box, Criterion, EngineModule, Error, KernelContext, Result (+3 more)

### Community 22 - "SIMD Arithmetic Tests"
Cohesion: 0.12
Nodes (15): Build and checks, code:mermaid (flowchart TB), code:text (plugins/theos-audio --> theos-plugin-api --> theos-shared + ), code:powershell (cargo build --workspace), code:powershell (git config core.hooksPath .githooks), Crate dependency graph (overview), Design principles, Entry points and apps (+7 more)

### Community 23 - "Module Lifecycle Protocol"
Cohesion: 0.09
Nodes (25): EventBus split into dyn-compatible core + generic ergonomic extension, Engine Module three-phase lifecycle (init, tick, shutdown), Receiver, Sender, Event, EventBus, EventBusExt, EventBusSetup (+17 more)

### Community 24 - "Entity Identity"
Cohesion: 0.22
Nodes (7): EntityId, u64, Display, FmtResult, Formatter, From, Self

### Community 25 - "Launcher & Log Filtering"
Cohesion: 0.38
Nodes (9): theos-launcher main, launcher log filtering test: defaults_to_info_when_rust_log_missing, launcher log filtering test: invalid_rust_log_does_not_crash_and_falls_back, choose_output(), module_path_is_present(), startup_line_matches_layout(), strip_ansi(), Output (+1 more)

### Community 26 - "Log Output Routing"
Cohesion: 0.46
Nodes (7): choose_output(), defaults_to_info_when_rust_log_missing(), invalid_rust_log_does_not_crash_and_falls_back(), strip_ansi(), Output, String, Vec

### Community 27 - "Tracing Field Visitor"
Cohesion: 0.40
Nodes (4): Field, FieldVisitor<'a, 'w>, Debug, Visit

### Community 28 - "Workspace Architecture Principles"
Cohesion: 0.50
Nodes (4): Strict DAG No-Circular-Dependency Crate Layout, theos-shared Zero-Internal-Dependency Policy, Workspace Design Principles (README), theos-shared EntityId Design

### Community 29 - "EventBus Send/Drain Benchmarks"
Cohesion: 0.83
Nodes (3): bench_send_then_drain_boxed_baseline(), bench_send_then_drain_typed(), Criterion

### Community 31 - "Service Discovery Test Fixtures"
Cohesion: 1.00
Nodes (3): ConsumerModule (test), GreetingService (test), ProviderModule (test)

### Community 51 - "Community 51"
Cohesion: 0.14
Nodes (13): code:powershell (cd "D:\Projects\Tower of Theos\theos-engine"), code:rust (// explicit import), code:mermaid (flowchart TB), Contribution notes, Design principles, EntityId (canonical decision), Open decisions / TODO, Planned public surface (+5 more)

### Community 52 - "Community 52"
Cohesion: 0.17
Nodes (11): AC traceability, Changes, Confirmed decisions (vs. spec wording), Goal, Other deltas vs. spec wording, Out of scope (deferred), Risks, Tests (`theos-core/src/kernel.rs` `mod tests`) (+3 more)

### Community 56 - "Community 56"
Cohesion: 0.14
Nodes (13): AddAssign, Div, theos-math lib (public API re-exports), Mat4 * Vec4, glam::Vec4, mint::Vector4<f32>, Vec4, Vec4Swizzle (+5 more)

## Knowledge Gaps
- **176 isolated node(s):** `Vec`, `Self`, `Self`, `Self`, `Option` (+171 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **17 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Vec3` connect `Vec3 & Quaternion Ops` to `Matrix Math & Benchmarks`, `Quaternion Arithmetic`, `AABB Spatial Queries`, `Vector Math & Interop Tests`, `Ray-Plane Intersection`, `SIMD & Interop Bridge Patterns`, `Vector Arithmetic Ops`, `Ray Traversal & BVH`, `Community 53`, `Community 56`?**
  _High betweenness centrality (0.038) - this node is a cross-community bridge._
- **Why does `EventBus` connect `Module Lifecycle Protocol` to `Core Shared Abstractions`?**
  _High betweenness centrality (0.020) - this node is a cross-community bridge._
- **Why does `KernelContext` connect `Core Shared Abstractions` to `Community 16`, `Module Lifecycle Protocol`?**
  _High betweenness centrality (0.020) - this node is a cross-community bridge._
- **Are the 20 inferred relationships involving `Aabb` (e.g. with `test_contains_point_interior()` and `test_contains_point_on_surface_is_inside()`) actually correct?**
  _`Aabb` has 20 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Vec`, `Self`, `Self` to the rest of the system?**
  _178 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Module Lifecycle & Kernel Fixtures` be split into smaller, more focused modules?**
  _Cohesion score 0.07138047138047138 - nodes in this community are weakly interconnected._
- **Should `Core Shared Abstractions` be split into smaller, more focused modules?**
  _Cohesion score 0.11806543385490754 - nodes in this community are weakly interconnected._