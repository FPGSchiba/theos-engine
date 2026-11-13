# Graph Report - theos-engine  (2026-08-29)

## Corpus Check
- 124 files · ~82,730 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1692 nodes · 3186 edges · 124 communities (87 shown, 37 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 33 edges (avg confidence: 0.83)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `05d88a23`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- kernel_integration.rs
- .sender_for_erased
- theos-core/src/event.rs
- dependency_resolver.rs
- TheosKernel
- mat.rs
- KernelError
- quat.rs
- aabb.rs
- kernel.rs
- .new
- CrossbeamEventBus
- Release Packaging Job
- upkeep.rs
- EntityId
- THEOS-86: Microkernel Core Bootstrap — Entry Point, Thread Pool & Kernel Initialization
- ServiceLocator
- Vec4
- Self
- TheosFormatter
- THEOS-158 — [magic] Injection + E4 interference amplitude (A_j)
- .init
- theos-engine
- EscapeToQuitModule
- KernelContext
- theos-launcher main
- choose_output
- .record_debug
- Strict DAG No-Circular-Dependency Crate Layout
- benches/event_bus.rs
- .sender_for_erased
- EventBus
- EngineModule
- chakra.rs
- RecordingModule<Tag>
- .run
- CallLog
- Chakra
- log_format.rs
- WindowError
- theos-audio/src/lib.rs
- theos-assets/src/lib.rs
- theos-ecs/src/lib.rs
- theos-plugin-api/src/lib.rs
- theos-plugin-loader/src/lib.rs
- theos-render/src/lib.rs
- theos-scripting lib (add placeholder)
- theos-game main
- theos-audio lib (placeholder)
- Mentor Agent: Tiered Escalation Protocol
- theos-shared
- THEOS-12: Initialization Phase — extract `Kernel::init()`
- CloseToExitModule
- theos-scripting/src/lib.rs
- WeaveTuning
- NodeId
- Test Coverage Improvement Plan — 2026-06-08
- glam interoperability bridge pattern
- WindowModule
- THEOS-15: Window Module Wrapper
- pre-commit
- FrameTimer
- THEOS-16: Input Event Propagation
- theos-engine
- TheosService
- BVH / ray-traversal usage pattern for Aabb and Ray
- THEOS-147 — [seam] ECS World access trait + in-memory stub + AI components
- EventBus split into dyn-compatible core + generic ergonomic extension
- theos-window
- KernelContext is a concrete struct (not a trait) for type-safe service access
- strip_ansi
- Engine Module three-phase lifecycle (init, tick, shutdown)
- Service Locator pattern for engine subsystem registry
- Chakras
- ElementField
- window_event.rs
- THEOS-148 — [M1][magic] theos-magic crate scaffold & core HarmonicToken seam
- 16-byte SIMD alignment for GPU/SIMD compatibility
- theos-ai
- assembler.rs
- theos-assets
- Locked decisions (design session 2026-06-13)
- Vec3
- theos-audio
- THEOS-157 — [M2][magic] TemporalFrequency → transfer element (E12)
- theos-ecs
- theos-editor
- THEOS-156 — Cast-time from critical-path propagation delay (E13)
- THEOS-150 — [M3] resonance stability sparse solver (E3/E5/E6/E7)
- THEOS-159 — E9 energy + E10 efficiency (η)
- THEOS-173 — [M2] Cast-time energy-throughput floor (Option B): `E_cost / P`
- Element Cosmology — 2-D (frequency × phase-polarity)
- THEOS-171 — [formalism] Pin physical-`f` → `f_norm` normalization on Shared Formalism
- theos-game
- 🎯 Sprint Planning Summit — Goal
- stability.rs
- cast.rs
- energy.rs
- THEOS-177 — [magic] B2 overdraw (channel beyond safe P/Cᵢ — demonic speed)
- math_benches.rs
- theos-magic
- theos-math
- THEOS-176 — [magic] E15 resonance gain (Q = 1/Sᵞ)
- theos-physics
- theos-plugin-api
- theos-plugin-loader
- theos-render
- theos-scripting
- theos-window
- theos-math lib (public API re-exports)
- Mat3 * Vec3
- Mat4 * Vec4
- Quat * Vec3 (Rodrigues rotation)
- THEOS-180 — [magic] E16a/b upkeep rate + depth-attenuated throughput occupation
- key_translate.rs
- window_factory.rs
- TheosAppHandler
- WindowEventTranslator

## God Nodes (most connected - your core abstractions)
1. `Vec3` - 89 edges
2. `TheosKernel` - 38 edges
3. `Vec4` - 38 edges
4. `Vec2` - 34 edges
5. `assemble()` - 32 edges
6. `Chakras` - 31 edges
7. `WovenToken` - 27 edges
8. `KernelContext` - 27 edges
9. `ChakraState` - 24 edges
10. `WeaveTuning` - 23 edges

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

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Unsafe Code Audit Triad (Clippy, Forbid, Geiger)** — github_workflows_check_lint_clippy_unsafe_policy_job, github_workflows_check_lint_forbid_unsafe_verification, github_workflows_check_lint_cargo_geiger [EXTRACTED 0.95]
- **Cross-Module Service Discovery via KernelContext: ProviderModule registers, ConsumerModule retrieves** — theos_core_tests_kernel_integration_providermodule, theos_core_tests_kernel_integration_consumermodule, theos_core_tests_kernel_integration_greetingservice [EXTRACTED 1.00]
- **Type-Erased EventBus: CrossbeamEventBus stores ChannelHandle<TypedChannel<E>> keyed by TypeId** — src_lib_crossbeameventbus, src_lib_channelhandle, theos_core_src_event_typedchannel_e [EXTRACTED 1.00]
- **Kernel Lifecycle: Register → DependencyResolver sort → init → fixed-step loop → shutdown** — theos_core_src_kernel_theoskernel, theos_core_src_dependency_resolver_dependencyresolver, concept_fixed_step_loop [EXTRACTED 1.00]
- **Release Artifact Production (Launcher, Game, Editor)** — github_workflows_release_packaging_theos_launcher_artifact, github_workflows_release_packaging_theos_game_artifact, github_workflows_release_packaging_theos_editor_artifact [EXTRACTED 1.00]
- **CI Quality Gate Pipeline (Check, Lint, Test)** — github_workflows_check_lint_check_lint_workflow, github_workflows_test_test_workflow, github_workflows_check_lint_clippy_unsafe_policy_job [INFERRED 0.85]

## Communities (124 total, 37 thin omitted)

### Community 0 - "kernel_integration.rs"
Cohesion: 0.08
Nodes (30): ConsumerModule, CountingModule, CountingModule<Tag>, GreetingService, kernel_runs_init_tick_shutdown_lifecycle(), make_counting_module(), module_retrieves_service_registered_by_its_dependency(), ProviderModule (+22 more)

### Community 1 - ".sender_for_erased"
Cohesion: 0.35
Nodes (7): Any, Box, EventBusResult, Option, Send, TypeId, StubBus

### Community 2 - "theos-core/src/event.rs"
Cohesion: 0.10
Nodes (9): Sender, BroadcastEventBus, Debug, E, Formatter, HashMap, Result, RwLock (+1 more)

### Community 3 - "dependency_resolver.rs"
Cohesion: 0.07
Nodes (29): Kahn's Topological Sort for Module Init Order, ModuleResult, DependencyResolver, five_module_mixed_graph_satisfies_all_dependencies(), linear_chain_resolves_in_dependency_order(), module(), module_with_deps(), Box (+21 more)

### Community 4 - "TheosKernel"
Cohesion: 0.20
Nodes (8): event_bus_handle_returns_clone_of_kernel_bus(), Arc, AtomicU32, Default, Option, target_hz_handle_shares_state_with_target_hz(), TheosKernel, ThreadPool

### Community 5 - "mat.rs"
Cohesion: 0.06
Nodes (46): glam::Mat3, glam::Mat4, Mat3, Mat4, From, Mul, Output, Self (+38 more)

### Community 6 - "KernelError"
Cohesion: 0.05
Nodes (41): KernelError, module_error_converts_to_theos_error_wrapped(), ModuleError, Box, Display, Error, FmtResult, Formatter (+33 more)

### Community 7 - "quat.rs"
Cohesion: 0.15
Nodes (22): assert_quat_eq(), assert_vec3_eq(), Quat, rot90_z(), Mul, MulAssign, Output, Self (+14 more)

### Community 8 - "aabb.rs"
Cohesion: 0.11
Nodes (12): Aabb, assert_vec_eq(), Self, test_expand_to_contain_point_inside_is_noop(), test_expand_to_contain_point_outside(), test_merge_produces_bounding_box(), test_new_degenerate_point_box_is_valid(), test_new_stores_corners() (+4 more)

### Community 9 - "kernel.rs"
Cohesion: 0.19
Nodes (24): calling_init_twice_returns_already_initialized(), default_constructs_kernel_with_no_modules_and_uninitialised_flag(), init_calls_module_init_in_dependency_sorted_order(), init_propagates_dependency_resolver_error_without_running_any_module(), init_stops_at_first_failing_module_and_skips_later_modules(), init_with_no_modules_succeeds_and_marks_kernel_initialised(), registering_duplicate_name_errors_and_leaves_buffer_unchanged(), registering_three_distinct_modules_fills_staging_buffer() (+16 more)

### Community 10 - ".new"
Cohesion: 0.16
Nodes (21): test_glam_vec2_roundtrip(), test_glam_vec3_and_vec3a_produce_same_components(), test_glam_vec3_roundtrip(), test_glam_vec3a_roundtrip(), test_glam_vec4_roundtrip(), test_vec2_constructors_and_dot(), test_vec2_dot_antiparallel_is_negative(), test_vec2_dot_orthogonal_is_zero() (+13 more)

### Community 11 - "CrossbeamEventBus"
Cohesion: 0.36
Nodes (8): Fixed-Step Game Loop (60 Hz sleep+spin hybrid), Service Locator encapsulated in KernelContext, Type-Erased EventBus via ChannelHandle adapter, CrossbeamEventBus, THEOS-86 Microkernel Bootstrap Plan, allocation_check_typed_event_bus (alloc test), bench_send_then_drain_typed (Criterion benchmark), GameEvent

### Community 12 - "Release Packaging Job"
Cohesion: 0.09
Nodes (23): Cargo Plugins Workspace Dependency Updates (theos-plugin-deps), Cargo Workspace Dependency Updates (theos-engine-deps), Dependabot Configuration, GitHub Actions Dependency Updates, cargo-geiger Unsafe Audit Tool, Cargo Check Job, Check and Lint Workflow, Clippy and Unsafe Policy Job (+15 more)

### Community 13 - "upkeep.rs"
Cohesion: 0.22
Nodes (27): approx(), assemble(), body(), occ(), Option, Vec, same_construct_two_environments_differ_only_in_upkeep_rate(), spatial() (+19 more)

### Community 14 - "EntityId"
Cohesion: 0.05
Nodes (47): AtomicU64, EntityMap, AiBrain, PendingAiResponse, String, add(), it_works(), component_ids_lists_inserted_component_types() (+39 more)

### Community 15 - "THEOS-86: Microkernel Core Bootstrap — Entry Point, Thread Pool & Kernel Initialization"
Cohesion: 0.04
Nodes (44): 1.1 Define `EngineModule` Trait (in `theos-shared`), 1.2 Define `EventBus` Trait (in `theos-shared`), 1.3 Create `ServiceLocator` Struct (in `theos-shared`), 1.4 Stub Out `TheosKernel` Struct (in `theos-core`), 1.5 Define `AiWorkerHandle` Struct (in `theos-core`), 1.6 Define `AiCommandQueue` Struct (in `theos-core`), 1.7 Design `ModuleRegistry` (Logical Component), 2.1 Logging Initialization (+36 more)

### Community 16 - "ServiceLocator"
Cohesion: 0.17
Nodes (14): ServiceLocatorResult, register_and_get_service(), Any, Arc, HashMap, Option, RwLock, Send (+6 more)

### Community 17 - "Vec4"
Cohesion: 0.15
Nodes (8): glam::Vec3A, glam::Vec4, mint::Vector4<f32>, From, Vec4, Vec4Swizzle, wide::f32x4, Vector4

### Community 18 - "Self"
Cohesion: 0.15
Nodes (3): Output, Self, test_simd_interop()

### Community 19 - "TheosFormatter"
Cohesion: 0.15
Nodes (13): FmtContext, FormatEvent, N, Rfc3339, S, logging::init(), FieldVisitor, init() (+5 more)

### Community 20 - "THEOS-158 — [magic] Injection + E4 interference amplitude (A_j)"
Cohesion: 0.22
Nodes (8): AC → file gap table (provisional — firms up as Q1–Q5 settle), Canon anchors (from Equations of the Weave, E4), Coverage target, Deferred / out of scope, Locked decisions (planning session 2026-06-14), Open design questions (to resolve with the user before coding), Scaffolding contract, THEOS-158 — [magic] Injection + E4 interference amplitude (A_j)

### Community 21 - ".init"
Cohesion: 0.17
Nodes (11): bench_tick_dispatch(), NoOpModule, NoOpModule<TAG>, register_n_modules(), Box, Criterion, Error, Result (+3 more)

### Community 22 - "theos-engine"
Cohesion: 0.12
Nodes (15): Build and checks, code:mermaid (flowchart TB), code:text (plugins/theos-audio --> theos-plugin-api --> theos-shared + ), code:powershell (cargo build --workspace), code:powershell (git config core.hooksPath .githooks), Crate dependency graph (overview), Design principles, Entry points and apps (+7 more)

### Community 23 - "EscapeToQuitModule"
Cohesion: 0.15
Nodes (18): EscapeToQuitModule, make_initialised_escape_module(), Arc, Box, Default, Error, Option, Receiver (+10 more)

### Community 24 - "KernelContext"
Cohesion: 0.23
Nodes (12): KernelContextResult, event_bus_hands_back_the_same_shared_handle(), get_unregistered_service_returns_service_not_found(), KernelContext, make_context(), register_overwrites_previous_service_of_same_type(), register_then_get_returns_the_service(), Arc (+4 more)

### Community 25 - "theos-launcher main"
Cohesion: 1.00
Nodes (3): theos-launcher main, launcher log filtering test: defaults_to_info_when_rust_log_missing, launcher log filtering test: invalid_rust_log_does_not_crash_and_falls_back

### Community 26 - "choose_output"
Cohesion: 0.46
Nodes (7): choose_output(), defaults_to_info_when_rust_log_missing(), invalid_rust_log_does_not_crash_and_falls_back(), Output, String, Vec, strip_ansi()

### Community 27 - ".record_debug"
Cohesion: 0.40
Nodes (4): Field, FieldVisitor<'a, 'w>, Debug, Visit

### Community 28 - "Strict DAG No-Circular-Dependency Crate Layout"
Cohesion: 0.50
Nodes (4): Strict DAG No-Circular-Dependency Crate Layout, theos-shared Zero-Internal-Dependency Policy, Workspace Design Principles (README), theos-shared EntityId Design

### Community 29 - "benches/event_bus.rs"
Cohesion: 0.83
Nodes (3): bench_send_then_recv_boxed_baseline(), bench_send_then_recv_typed(), Criterion

### Community 30 - ".sender_for_erased"
Cohesion: 0.31
Nodes (9): ChannelHandle (private trait), Any, Box, EventBusResult, Option, Send, Sync, TypeId (+1 more)

### Community 31 - "EventBus"
Cohesion: 0.18
Nodes (14): BroadcastBusExt, T, Test2Event, TestEvent, PhysicsEvent, ChannelHandle, Event, EventBus (+6 more)

### Community 32 - "EngineModule"
Cohesion: 0.22
Nodes (8): FailingInitModule, EngineModule, ModuleDependency, Self, Send, Sync, TypeId, VersionReq

### Community 33 - "chakra.rs"
Cohesion: 0.22
Nodes (8): chakra_state_round_trips_through_json(), chakras_default_is_unattuned_and_balanced(), from_array_preserves_canonical_order(), indexing_and_iteration_observe_the_same_state(), iter_mut_allows_mutation_of_every_point(), Polarity, Default, Self

### Community 34 - "RecordingModule<Tag>"
Cohesion: 0.17
Nodes (6): RecordingModule<Tag>, Error, Result, Send, Sync, TypeId

### Community 35 - ".run"
Cohesion: 0.29
Nodes (5): KernelResult, run_returns_uninitialized_error_when_init_was_not_called(), shutdown_all_before_init_returns_uninitialized_error(), step_after_shutdown_all_returns_already_shutdown_error(), step_before_init_returns_uninitialized_error()

### Community 36 - "CallLog"
Cohesion: 0.31
Nodes (6): CallLog, RecordingModule, Mutex, PhantomData, Tag, Vec

### Community 37 - "Chakra"
Cohesion: 0.33
Nodes (3): Chakra, Item, Iterator

### Community 38 - "log_format.rs"
Cohesion: 0.57
Nodes (6): choose_output(), module_path_is_present(), Output, String, startup_line_matches_layout(), strip_ansi()

### Community 39 - "WindowError"
Cohesion: 0.50
Nodes (3): OsError, EventLoopError, WindowError

### Community 51 - "theos-shared"
Cohesion: 0.13
Nodes (13): code:powershell (cd "D:\Projects\Tower of Theos\theos-engine"), code:rust (// explicit import), code:mermaid (flowchart TB), Contribution notes, Design principles, EntityId (canonical decision), Open decisions / TODO, Planned public surface (+5 more)

### Community 52 - "THEOS-12: Initialization Phase — extract `Kernel::init()`"
Cohesion: 0.17
Nodes (11): AC traceability, Changes, Confirmed decisions (vs. spec wording), Goal, Other deltas vs. spec wording, Out of scope (deferred), Risks, Tests (`theos-core/src/kernel.rs` `mod tests`) (+3 more)

### Community 53 - "CloseToExitModule"
Cohesion: 0.09
Nodes (29): CloseToExitModule, make_initialised_close_module(), Arc, Box, Default, Error, Option, Receiver (+21 more)

### Community 56 - "WeaveTuning"
Cohesion: 0.10
Nodes (39): approx(), gain_differs_with_environment_on_same_graph(), gain_matches_manual_q_for_known_s_and_gamma(), gain_near_one_for_well_damped_graph(), gain_rises_as_stability_margin_thins(), ResonanceRunaway, Result, runaway_when_stability_nonpositive() (+31 more)

### Community 57 - "NodeId"
Cohesion: 0.08
Nodes (29): Arena, Arena<T>, default_arena_is_empty(), get_mut_allows_in_place_update(), get_returns_none_for_stale_handle_after_remove(), insert_reuses_freed_slot_with_bumped_generation(), insert_then_get_returns_the_value(), len_and_is_empty_track_live_values() (+21 more)

### Community 58 - "Test Coverage Improvement Plan — 2026-06-08"
Cohesion: 0.05
Nodes (39): Coverage targets, graphify, Manual smoke, Plans, Pre-commit checks, Quality gates, Test scaffolding pattern, Theos-Engine (+31 more)

### Community 60 - "WindowModule"
Cohesion: 0.20
Nodes (9): Arc, Box, Error, Result, Self, Send, Sync, Window (+1 more)

### Community 61 - "THEOS-15: Window Module Wrapper"
Cohesion: 0.12
Nodes (15): Acceptance-criteria traceability, Confirmed decisions (locked, see concept page), Frame timer (`theos-core::frame_timer`), FrameTimer — covered via existing kernel tests, Kernel — automated unit tests, Kernel (`theos-core`), Launcher (`theos-launcher`), Out of scope (deferred) (+7 more)

### Community 63 - "FrameTimer"
Cohesion: 0.31
Nodes (7): default_returns_first_frame_timer(), FrameTimer, Default, Instant, Option, Self, second_tick_returns_real_delta()

### Community 64 - "THEOS-16: Input Event Propagation"
Cohesion: 0.15
Nodes (12): Architecture sub-question (decide before AC2), Locked decisions, Option A — `ShutdownRequester` service in `KernelContext`, Option B — `ShutdownRequested` event on the bus, Out of scope (deferred — match the Jira), Remaining implementation work (logic, not scaffolding), Summary, THEOS-16: Input Event Propagation (+4 more)

### Community 65 - "theos-engine"
Cohesion: 0.17
Nodes (11): Build and checks, Crate dependency graph (overview), Design principles, Entry points and apps, Foundation, Kernel and subsystems, Notes, Plugin and scripting boundaries (+3 more)

### Community 66 - "TheosService"
Cohesion: 0.23
Nodes (10): Deref, Target, Sync, TestService, TheosService, UnregisteredService, Arc, Self (+2 more)

### Community 70 - "THEOS-147 — [seam] ECS World access trait + in-memory stub + AI components"
Cohesion: 0.22
Nodes (8): Acceptance-criteria gap table, Crate wiring (done), Deferred / out of scope, Locked decisions, Related, Resolved design decisions (were open during design), Test execution / CI, THEOS-147 — [seam] ECS World access trait + in-memory stub + AI components

### Community 72 - "theos-window"
Cohesion: 0.33
Nodes (5): Build, Platform requirements, Public API, Testing, theos-window

### Community 74 - "strip_ansi"
Cohesion: 0.67
Nodes (3): full_lifecycle_emits_expected_log_sequence(), String, strip_ansi()

### Community 78 - "Chakras"
Cohesion: 0.21
Nodes (6): Chakras, ChakraState, named_accessors_reach_every_point(), Index, IndexMut, Option

### Community 79 - "ElementField"
Cohesion: 0.11
Nodes (11): Attunement, CompilerContext, element_field_round_trips_through_json(), ElementField, ElementKind, Index, IndexMut, Item (+3 more)

### Community 80 - "window_event.rs"
Cohesion: 0.19
Nodes (6): PhysicalKey, KeyCode, NativeKeyCode, WindowEvent, translate_native(), translate_physical_key()

### Community 81 - "THEOS-148 — [M1][magic] theos-magic crate scaffold & core HarmonicToken seam"
Cohesion: 0.29
Nodes (6): Acceptance-criteria gap table, Deferred, Locked decisions, Resolved decisions (chakra & element model), Test execution, THEOS-148 — [M1][magic] theos-magic crate scaffold & core HarmonicToken seam

### Community 84 - "assembler.rs"
Cohesion: 0.05
Nodes (64): CompilerError, String, approx(), assemble(), assemble_default_uses_the_default_wave_speed(), body(), cast_time_equals_critical_path_delay_when_no_transfers_present(), cast_time_on_acyclic_graph_matches_manual_calc() (+56 more)

### Community 86 - "Locked decisions (design session 2026-06-13)"
Cohesion: 0.17
Nodes (11): AC → file gap table, Chakra layer (`chakra.rs`), Coverage target, Deferred / out of scope, Graph layer (`graph/`), Locked decisions (design session 2026-06-13), Scaffolding contract, Seam ripple (`interface.rs`, `error.rs`) (+3 more)

### Community 87 - "Vec3"
Cohesion: 0.08
Nodes (20): Add, AddAssign, Div, DivAssign, f32x4, Sub, SubAssign, f32 (+12 more)

### Community 89 - "THEOS-157 — [M2][magic] TemporalFrequency → transfer element (E12)"
Cohesion: 0.25
Nodes (7): Acceptance criteria, Coverage, Deferred / follow-ups, How the scope changed (design journey, 2026-06-14), Implementation map, Locked equations (E12), THEOS-157 — [M2][magic] TemporalFrequency → transfer element (E12)

### Community 92 - "THEOS-156 — Cast-time from critical-path propagation delay (E13)"
Cohesion: 0.33
Nodes (5): AC mapping, Deferred / out of scope, Implementation locus, Locked decisions, THEOS-156 — Cast-time from critical-path propagation delay (E13)

### Community 93 - "THEOS-150 — [M3] resonance stability sparse solver (E3/E5/E6/E7)"
Cohesion: 0.12
Nodes (16): Acceptance criteria → coverage, Canon (pinned from *Equations of the Weave*, page 170164225), Cross-story notes, Current code state (`theos-magic/src/graph`), D1 — Where do `kᵢⱼ`, `Lᵢⱼ`, `σ_f` live?  ⚠️ architecture (the ticket's flagged call), D2 — Default values (no token supplies these yet), D3 — `σ_f` (frequency tolerance) source, D4 — Authoring/execution split for `S` (mirror 159 D5)  ⚠️ keep consistent (+8 more)

### Community 94 - "THEOS-159 — E9 energy + E10 efficiency (η)"
Cohesion: 0.12
Nodes (16): Acceptance criteria → coverage, Canon (pinned from *Shared Formalism: Equations of the Weave*, page 170164225), Current code state (grounding — `theos-magic/src/graph`), D1 — What is the per-edge amplitude `Aᵢⱼ`?  ⚠️ core, D2 — Where do `ρ_med` and `ρ_env` come from? (not in the data model today), D3 — Which nodes are "effectors" for the E10 numerator?  ⚠️ core, D4 — η when `E_cost == 0` (no-injection / zero-energy graph)? (AC3), D5 — Authoring-side (frozen) or execution-side (env-dependent)?  ⚠️ architecture, couples to THEOS-173 (+8 more)

### Community 95 - "THEOS-173 — [M2] Cast-time energy-throughput floor (Option B): `E_cost / P`"
Cohesion: 0.12
Nodes (15): Acceptance criteria, Canon state, Current code state, Decision register — **LOCKED** (2026-06-21), DP1 — **What IS `P`?**  ⚠️ worldbuilding — your call (no canon today), DP2 — Aggregation (if `P` derives from chakras), DP3 — `effective_cast_time` seam (scaffolding), DP4 — `P ≤ 0` guard (+7 more)

### Community 96 - "Element Cosmology — 2-D (frequency × phase-polarity)"
Cohesion: 0.17
Nodes (11): Axis 1 — Frequency `f` (E1): the *material* spectrum — **LOCKED: logarithmic**, Axis 2 — Phase coherence: **Light ↔ Darkness** polarity — **curve: sigmoid (confirm)**, Canon impact (edits this implies), Cultivation & resource economy — DESIGN STUB (own ticket; NOT in the canon prompts), Decisions (LOCKED 2026-06-21 unless noted), Element Cosmology — 2-D (frequency × phase-polarity), Goal, Relationship to sprint stories (+3 more)

### Community 97 - "THEOS-171 — [formalism] Pin physical-`f` → `f_norm` normalization on Shared Formalism"
Cohesion: 0.18
Nodes (10): Acceptance criteria, Decision register — **OPEN** (two coupled worldbuilding calls), Delivery, DN1 — Range `[f_min, f_max]` (the "authored frequency range", Hz), DN2 — Curve (linear vs logarithmic), DN3 — Clamp, Goal (one sentence), Grounded context (from *Core UX: Spell Appearance*, page 177897474, §3–4) (+2 more)

### Community 99 - "🎯 Sprint Planning Summit — Goal"
Cohesion: 0.40
Nodes (4): Current focus, Sequence (decided 2026-06-21), 🎯 Sprint Planning Summit — Goal, Success criteria

### Community 100 - "stability.rs"
Cohesion: 0.19
Nodes (23): approx(), assemble(), body(), coherence_high_for_in_phase_frequency_matched_edge(), coherence_low_for_divergent_frequencies_via_sigma_f(), constructive_graph_is_stable(), cyclic_feedback_runaway_is_volatile(), div() (+15 more)

### Community 101 - "cast.rs"
Cohesion: 0.26
Nodes (19): approx(), CannotChannel, caster(), caster_with_root_power(), effective_cast_time_is_energy_time_when_throughput_bound(), effective_cast_time_is_settling_when_propagation_bound(), frozen_cast_time_unchanged_by_floor(), overdraw_past_capacity_snaps_the_anchor() (+11 more)

### Community 102 - "energy.rs"
Cohesion: 0.26
Nodes (16): approx(), assemble(), body(), efficiency_matches_manual_eta_on_constructive_graph(), efficiency_on_zero_energy_graph_is_well_defined(), EnvResistance, full_transfer_graph_efficiency_near_one(), graph_energy_matches_manual_sum_over_edges() (+8 more)

### Community 103 - "THEOS-177 — [magic] B2 overdraw (channel beyond safe P/Cᵢ — demonic speed)"
Cohesion: 0.20
Nodes (9): Acceptance criteria, Canon (E14 B2, landed 2026-06-25), Current code state, Decision register — LOCKED (2026-06-25), Goal (one sentence), Out of scope, Proposed shape (scaffolding only), Test scenarios (names only) (+1 more)

### Community 104 - "math_benches.rs"
Cohesion: 0.60
Nodes (5): bench_mat3_mul(), bench_mat4_mul(), bench_vec3_cross(), bench_vec3_dot(), Criterion

### Community 107 - "THEOS-176 — [magic] E15 resonance gain (Q = 1/Sᵞ)"
Cohesion: 0.22
Nodes (8): Acceptance criteria, Canon (E15, Equations of the Weave), Decision register — LOCKED (2026-06-25), Goal (one sentence), Out of scope, Proposed shape (scaffolding only), Test scenarios (names only), THEOS-176 — [magic] E15 resonance gain (Q = 1/Sᵞ)

### Community 119 - "THEOS-180 — [magic] E16a/b upkeep rate + depth-attenuated throughput occupation"
Cohesion: 0.17
Nodes (11): Acceptance criteria (from Jira), Canon (Equations of the Weave, E16a/b), Coverage note, Current code state (all inputs already exist), Decision register — LOCKED (2026-08-29, guided session), Goal (one sentence), Implementation notes (2026-08-29), Out of scope / deferred (+3 more)

### Community 120 - "key_translate.rs"
Cohesion: 0.26
Nodes (8): assert_maps(), translate_keycode_maps_all_digits(), translate_keycode_maps_all_letters(), translate_keycode_maps_editing_keys(), translate_keycode_maps_function_keys(), translate_keycode_maps_modifier_keys(), translate_keycode_maps_navigation_keys(), WinitKeyCode

### Community 121 - "window_factory.rs"
Cohesion: 0.29
Nodes (7): get_engine_version(), main(), create_default_window(), Arc, EventLoop, Window, WindowResult

### Community 122 - "TheosAppHandler"
Cohesion: 0.27
Nodes (7): Arc, AtomicBool, EventLoop, EventLoopError, Result, Self, TheosAppHandler

### Community 123 - "WindowEventTranslator"
Cohesion: 0.43
Nodes (4): Arc, Self, translator_constructs_with_event_bus_handle(), WindowEventTranslator

## Knowledge Gaps
- **312 isolated node(s):** `theos-audio`, `theos-ai`, `theos-assets`, `TagA`, `TagB` (+307 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **37 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `EntityId` connect `EntityId` to `KernelError`, `ElementField`?**
  _High betweenness centrality (0.286) - this node is a cross-community bridge._
- **Why does `TheosKernel` connect `TheosKernel` to `EngineModule`, `kernel_integration.rs`, `theos-core/src/event.rs`, `dependency_resolver.rs`, `CallLog`, `.run`, `KernelError`, `kernel.rs`, `CrossbeamEventBus`, `TheosFormatter`, `.init`, `CloseToExitModule`, `TheosAppHandler`?**
  _High betweenness centrality (0.224) - this node is a cross-community bridge._
- **Why does `KernelError` connect `KernelError` to `TheosKernel`?**
  _High betweenness centrality (0.214) - this node is a cross-community bridge._
- **What connects `theos-audio`, `theos-ai`, `theos-assets` to the rest of the system?**
  _312 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `kernel_integration.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.08048103607770583 - nodes in this community are weakly interconnected._
- **Should `theos-core/src/event.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.09881422924901186 - nodes in this community are weakly interconnected._
- **Should `dependency_resolver.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.07152496626180836 - nodes in this community are weakly interconnected._