# Graph Report - theos-engine  (2026-06-26)

## Corpus Check
- 102 files · ~77,532 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1843 nodes · 3611 edges · 106 communities (87 shown, 19 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 41 edges (avg confidence: 0.79)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `0a65c73a`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- [[_COMMUNITY_Community 0|Community 0]]
- [[_COMMUNITY_Community 1|Community 1]]
- [[_COMMUNITY_Community 2|Community 2]]
- [[_COMMUNITY_Community 3|Community 3]]
- [[_COMMUNITY_Community 4|Community 4]]
- [[_COMMUNITY_Community 5|Community 5]]
- [[_COMMUNITY_Community 6|Community 6]]
- [[_COMMUNITY_Community 7|Community 7]]
- [[_COMMUNITY_Community 8|Community 8]]
- [[_COMMUNITY_Community 9|Community 9]]
- [[_COMMUNITY_Community 10|Community 10]]
- [[_COMMUNITY_Community 11|Community 11]]
- [[_COMMUNITY_Community 12|Community 12]]
- [[_COMMUNITY_Community 13|Community 13]]
- [[_COMMUNITY_Community 14|Community 14]]
- [[_COMMUNITY_Community 15|Community 15]]
- [[_COMMUNITY_Community 16|Community 16]]
- [[_COMMUNITY_Community 17|Community 17]]
- [[_COMMUNITY_Community 18|Community 18]]
- [[_COMMUNITY_Community 19|Community 19]]
- [[_COMMUNITY_Community 20|Community 20]]
- [[_COMMUNITY_Community 21|Community 21]]
- [[_COMMUNITY_Community 22|Community 22]]
- [[_COMMUNITY_Community 23|Community 23]]
- [[_COMMUNITY_Community 24|Community 24]]
- [[_COMMUNITY_Community 25|Community 25]]
- [[_COMMUNITY_Community 26|Community 26]]
- [[_COMMUNITY_Community 27|Community 27]]
- [[_COMMUNITY_Community 28|Community 28]]
- [[_COMMUNITY_Community 29|Community 29]]
- [[_COMMUNITY_Community 30|Community 30]]
- [[_COMMUNITY_Community 31|Community 31]]
- [[_COMMUNITY_Community 32|Community 32]]
- [[_COMMUNITY_Community 33|Community 33]]
- [[_COMMUNITY_Community 34|Community 34]]
- [[_COMMUNITY_Community 35|Community 35]]
- [[_COMMUNITY_Community 36|Community 36]]
- [[_COMMUNITY_Community 37|Community 37]]
- [[_COMMUNITY_Community 38|Community 38]]
- [[_COMMUNITY_Community 39|Community 39]]
- [[_COMMUNITY_Community 40|Community 40]]
- [[_COMMUNITY_Community 41|Community 41]]
- [[_COMMUNITY_Community 42|Community 42]]
- [[_COMMUNITY_Community 43|Community 43]]
- [[_COMMUNITY_Community 47|Community 47]]
- [[_COMMUNITY_Community 48|Community 48]]
- [[_COMMUNITY_Community 49|Community 49]]
- [[_COMMUNITY_Community 50|Community 50]]
- [[_COMMUNITY_Community 51|Community 51]]
- [[_COMMUNITY_Community 52|Community 52]]
- [[_COMMUNITY_Community 53|Community 53]]
- [[_COMMUNITY_Community 54|Community 54]]
- [[_COMMUNITY_Community 56|Community 56]]
- [[_COMMUNITY_Community 57|Community 57]]
- [[_COMMUNITY_Community 58|Community 58]]
- [[_COMMUNITY_Community 59|Community 59]]
- [[_COMMUNITY_Community 60|Community 60]]
- [[_COMMUNITY_Community 61|Community 61]]
- [[_COMMUNITY_Community 62|Community 62]]
- [[_COMMUNITY_Community 63|Community 63]]
- [[_COMMUNITY_Community 64|Community 64]]
- [[_COMMUNITY_Community 65|Community 65]]
- [[_COMMUNITY_Community 66|Community 66]]
- [[_COMMUNITY_Community 67|Community 67]]
- [[_COMMUNITY_Community 68|Community 68]]
- [[_COMMUNITY_Community 69|Community 69]]
- [[_COMMUNITY_Community 70|Community 70]]
- [[_COMMUNITY_Community 71|Community 71]]
- [[_COMMUNITY_Community 72|Community 72]]
- [[_COMMUNITY_Community 73|Community 73]]
- [[_COMMUNITY_Community 74|Community 74]]
- [[_COMMUNITY_Community 75|Community 75]]
- [[_COMMUNITY_Community 78|Community 78]]
- [[_COMMUNITY_Community 79|Community 79]]
- [[_COMMUNITY_Community 80|Community 80]]
- [[_COMMUNITY_Community 81|Community 81]]
- [[_COMMUNITY_Community 82|Community 82]]
- [[_COMMUNITY_Community 83|Community 83]]
- [[_COMMUNITY_Community 84|Community 84]]
- [[_COMMUNITY_Community 85|Community 85]]
- [[_COMMUNITY_Community 86|Community 86]]
- [[_COMMUNITY_Community 87|Community 87]]
- [[_COMMUNITY_Community 88|Community 88]]
- [[_COMMUNITY_Community 89|Community 89]]
- [[_COMMUNITY_Community 90|Community 90]]
- [[_COMMUNITY_Community 91|Community 91]]
- [[_COMMUNITY_Community 92|Community 92]]
- [[_COMMUNITY_Community 93|Community 93]]
- [[_COMMUNITY_Community 94|Community 94]]
- [[_COMMUNITY_Community 95|Community 95]]
- [[_COMMUNITY_Community 96|Community 96]]
- [[_COMMUNITY_Community 97|Community 97]]
- [[_COMMUNITY_Community 98|Community 98]]
- [[_COMMUNITY_Community 99|Community 99]]
- [[_COMMUNITY_Community 100|Community 100]]
- [[_COMMUNITY_Community 101|Community 101]]
- [[_COMMUNITY_Community 102|Community 102]]
- [[_COMMUNITY_Community 103|Community 103]]
- [[_COMMUNITY_Community 104|Community 104]]
- [[_COMMUNITY_Community 107|Community 107]]

## God Nodes (most connected - your core abstractions)
1. `Vec3` - 60 edges
2. `Vec4` - 46 edges
3. `Self` - 42 edges
4. `Vec2` - 39 edges
5. `TheosKernel` - 35 edges
6. `assemble()` - 33 edges
7. `Aabb` - 32 edges
8. `ChakraState` - 23 edges
9. `Chakras` - 23 edges
10. `approx()` - 19 edges

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
- 1-file cycle: `theos-ai/src/components.rs -> theos-ai/src/components.rs`
- 1-file cycle: `theos-core/benches/event_bus.rs -> theos-core/benches/event_bus.rs`
- 1-file cycle: `theos-core/benches/frame_timing.rs -> theos-core/benches/frame_timing.rs`
- 1-file cycle: `theos-core/src/dependency_resolver.rs -> theos-core/src/dependency_resolver.rs`
- 1-file cycle: `theos-shared/src/module.rs -> theos-shared/src/module.rs`
- 1-file cycle: `theos-core/src/error.rs -> theos-core/src/error.rs`
- 1-file cycle: `theos-core/src/event.rs -> theos-core/src/event.rs`
- 1-file cycle: `theos-shared/src/event_bus.rs -> theos-shared/src/event_bus.rs`
- 1-file cycle: `theos-core/src/frame_timer.rs -> theos-core/src/frame_timer.rs`
- 1-file cycle: `theos-core/src/kernel.rs -> theos-core/src/kernel.rs`
- 1-file cycle: `theos-launcher/src/app_handler.rs -> theos-launcher/src/app_handler.rs`
- 1-file cycle: `theos-window/src/event_translator.rs -> theos-window/src/event_translator.rs`
- 1-file cycle: `theos-core/tests/kernel_integration.rs -> theos-core/tests/kernel_integration.rs`
- 1-file cycle: `theos-ecs/src/testkit.rs -> theos-ecs/src/testkit.rs`
- 1-file cycle: `theos-launcher/src/close_to_exit.rs -> theos-launcher/src/close_to_exit.rs`
- 1-file cycle: `theos-launcher/src/escape_to_quit.rs -> theos-launcher/src/escape_to_quit.rs`
- 1-file cycle: `theos-window/src/key_translate.rs -> theos-window/src/key_translate.rs`
- 1-file cycle: `theos-launcher/tests/log_filtering.rs -> theos-launcher/tests/log_filtering.rs`
- 1-file cycle: `theos-launcher/tests/log_format.rs -> theos-launcher/tests/log_format.rs`
- 1-file cycle: `theos-magic/src/chakra.rs -> theos-magic/src/chakra.rs`

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

## Communities (106 total, 19 thin omitted)

### Community 0 - "Community 0"
Cohesion: 0.08
Nodes (39): AtomicU32, Instant, Mutex, ConsumerModule, CountingModule, CountingModule<Tag>, GreetingService, kernel_runs_init_tick_shutdown_lifecycle() (+31 more)

### Community 1 - "Community 1"
Cohesion: 0.09
Nodes (41): EventBus split into dyn-compatible core + generic ergonomic extension, KernelContext is a concrete struct (not a trait) for type-safe service access, KernelContextResult, ServiceLocator, ChannelHandle, Event, EventBus, EventBusExt (+33 more)

### Community 2 - "Community 2"
Cohesion: 0.07
Nodes (36): EventBusSetup, Receiver, Sender, BroadcastBusExt, BroadcastEventBus, ChannelHandle, CrossbeamEventBus, GameEvent (+28 more)

### Community 3 - "Community 3"
Cohesion: 0.08
Nodes (43): Engine Module three-phase lifecycle (init, tick, shutdown), ModuleResult, DependencyResolver, empty_module_list_returns_empty_order(), five_module_mixed_graph_satisfies_all_dependencies(), linear_chain_resolves_in_dependency_order(), missing_dependency_returns_dependency_missing_error(), module() (+35 more)

### Community 4 - "Community 4"
Cohesion: 0.06
Nodes (68): BroadcastEventBus, CrossbeamEventBus, Default, KernelResult, calling_init_twice_returns_already_initialized(), CallLog, default_constructs_kernel_with_no_modules_and_uninitialised_flag(), event_bus_handle_returns_clone_of_kernel_bus() (+60 more)

### Community 5 - "Community 5"
Cohesion: 0.14
Nodes (27): theos-math lib (public API re-exports), glam::Mat3, glam::Mat4, Mat3, Mat4, test_glam_mat3_identity_roundtrip(), test_glam_mat3_roundtrip(), test_glam_mat4_identity_roundtrip() (+19 more)

### Community 6 - "Community 6"
Cohesion: 0.13
Nodes (23): display_external_variant_includes_prefix(), display_message_variant_is_human_readable(), EventBusError, from_string_and_str_create_message(), kernel_context_error_converts_into_theos_error(), kernel_context_service_not_found_displays_type_name_and_has_no_source(), kernel_context_wraps_service_locator_error_and_exposes_source(), KernelContextError (+15 more)

### Community 7 - "Community 7"
Cohesion: 0.18
Nodes (25): assert_quat_eq(), assert_vec3_eq(), Quat, rot90_z(), test_identity_components(), test_length_known_value(), test_length_of_identity_is_one(), test_mul_assign() (+17 more)

### Community 8 - "Community 8"
Cohesion: 0.19
Nodes (25): Aabb, assert_vec_eq(), test_contains_point_interior(), test_contains_point_on_surface_is_inside(), test_contains_point_outside(), test_expand_to_contain_point_inside_is_noop(), test_expand_to_contain_point_outside(), test_merge_is_symmetric() (+17 more)

### Community 9 - "Community 9"
Cohesion: 0.13
Nodes (23): EventBusError, KernelContextError, ServiceLocatorError, display_circular_dependency_mentions_cycle(), display_dependency_missing_mentions_both_parts(), display_initialization_failed_mentions_module_and_reason(), display_module_already_registered_is_human_readable(), display_module_not_found_is_human_readable() (+15 more)

### Community 10 - "Community 10"
Cohesion: 0.18
Nodes (25): test_glam_vec2_roundtrip(), test_glam_vec3_and_vec3a_produce_same_components(), test_glam_vec3_roundtrip(), test_glam_vec3a_roundtrip(), test_glam_vec4_roundtrip(), test_mint_interop(), test_simd_interop(), test_vec2_constructors_and_dot() (+17 more)

### Community 11 - "Community 11"
Cohesion: 0.11
Nodes (22): Fixed-Step Game Loop (60 Hz sleep+spin hybrid), Kahn's Topological Sort for Module Init Order, Service Locator encapsulated in KernelContext, Type-Erased EventBus via ChannelHandle adapter, allocation_check_typed_event_bus (alloc test), bench_send_then_drain_typed (Criterion benchmark), bench_tick_dispatch (Criterion benchmark), NoOpModule (bench) (+14 more)

### Community 12 - "Community 12"
Cohesion: 0.09
Nodes (23): Cargo Plugins Workspace Dependency Updates (theos-plugin-deps), Cargo Workspace Dependency Updates (theos-engine-deps), Dependabot Configuration, GitHub Actions Dependency Updates, cargo-geiger Unsafe Audit Tool, Cargo Check Job, Check and Lint Workflow, Clippy and Unsafe Policy Job (+15 more)

### Community 13 - "Community 13"
Cohesion: 0.14
Nodes (30): BVH / ray-traversal usage pattern for Aabb and Ray, Ray, assert_vec3_eq(), Plane, test_intersects_ray_from_above_hits_origin(), test_intersects_ray_from_below_hits_plane(), test_intersects_ray_offset_origin(), test_intersects_ray_parallel_returns_none() (+22 more)

### Community 14 - "Community 14"
Cohesion: 0.12
Nodes (28): AtomicU64, ComponentValue, EntityId, EntityMap, component_ids_lists_inserted_component_types(), contains_is_true_for_spawned_entity(), despawn_removes_entity_and_its_components(), get_errors_with_component_not_found_when_component_absent() (+20 more)

### Community 15 - "Community 15"
Cohesion: 0.04
Nodes (44): 1.1 Define `EngineModule` Trait (in `theos-shared`), 1.2 Define `EventBus` Trait (in `theos-shared`), 1.3 Create `ServiceLocator` Struct (in `theos-shared`), 1.4 Stub Out `TheosKernel` Struct (in `theos-core`), 1.5 Define `AiWorkerHandle` Struct (in `theos-core`), 1.6 Define `AiCommandQueue` Struct (in `theos-core`), 1.7 Design `ModuleRegistry` (Logical Component), 2.1 Logging Initialization (+36 more)

### Community 16 - "Community 16"
Cohesion: 0.17
Nodes (17): Service Locator pattern for engine subsystem registry, RwLock, ServiceLocatorResult, get_nonexistent_service_returns_none(), register_and_get_service(), service_locator_is_cloneable_and_shares_registry(), ServiceLocator, TestService (+9 more)

### Community 17 - "Community 17"
Cohesion: 0.21
Nodes (11): Mat3 * Vec3, Quat * Vec3 (Rodrigues rotation), f32, glam::Vec3, glam::Vec3A, mint::Vector3<f32>, Vec3, From (+3 more)

### Community 18 - "Community 18"
Cohesion: 0.11
Nodes (3): wide::f32x4, Output, Self

### Community 19 - "Community 19"
Cohesion: 0.16
Nodes (13): FmtContext, FormatEvent, N, Rfc3339, S, FieldVisitor, init(), TheosFormatter (+5 more)

### Community 20 - "Community 20"
Cohesion: 0.22
Nodes (8): AC → file gap table (provisional — firms up as Q1–Q5 settle), Canon anchors (from Equations of the Weave, E4), Coverage target, Deferred / out of scope, Locked decisions (planning session 2026-06-14), Open design questions (to resolve with the user before coding), Scaffolding contract, THEOS-158 — [magic] Injection + E4 interference amplitude (A_j)

### Community 21 - "Community 21"
Cohesion: 0.16
Nodes (14): bench_tick_dispatch(), NoOpModule, NoOpModule<TAG>, register_n_modules(), Box, Criterion, EngineModule, Error (+6 more)

### Community 22 - "Community 22"
Cohesion: 0.12
Nodes (15): Build and checks, code:mermaid (flowchart TB), code:text (plugins/theos-audio --> theos-plugin-api --> theos-shared + ), code:powershell (cargo build --workspace), code:powershell (git config core.hooksPath .githooks), Crate dependency graph (overview), Design principles, Entry points and apps (+7 more)

### Community 23 - "Community 23"
Cohesion: 0.12
Nodes (24): EscapeToQuitModule, make_initialised_escape_module(), tick_ignores_close_requested(), tick_ignores_key_released_for_escape(), tick_ignores_non_escape_keypresses(), tick_is_noop_before_init(), tick_requests_shutdown_on_escape_press(), tick_with_empty_receiver_is_idempotent() (+16 more)

### Community 24 - "Community 24"
Cohesion: 0.14
Nodes (7): EntityId, u64, Display, FmtResult, Formatter, From, Self

### Community 25 - "Community 25"
Cohesion: 0.40
Nodes (9): theos-launcher main, launcher log filtering test: defaults_to_info_when_rust_log_missing, launcher log filtering test: invalid_rust_log_does_not_crash_and_falls_back, choose_output(), module_path_is_present(), startup_line_matches_layout(), strip_ansi(), Output (+1 more)

### Community 26 - "Community 26"
Cohesion: 0.50
Nodes (7): choose_output(), defaults_to_info_when_rust_log_missing(), invalid_rust_log_does_not_crash_and_falls_back(), strip_ansi(), Output, String, Vec

### Community 27 - "Community 27"
Cohesion: 0.40
Nodes (4): Field, FieldVisitor<'a, 'w>, Debug, Visit

### Community 28 - "Community 28"
Cohesion: 0.50
Nodes (4): Strict DAG No-Circular-Dependency Crate Layout, theos-shared Zero-Internal-Dependency Policy, Workspace Design Principles (README), theos-shared EntityId Design

### Community 29 - "Community 29"
Cohesion: 0.48
Nodes (5): bench_send_then_drain_boxed_baseline(), bench_send_then_drain_typed(), bench_send_then_recv_boxed_baseline(), bench_send_then_recv_typed(), Criterion

### Community 30 - "Community 30"
Cohesion: 0.23
Nodes (7): get_engine_version(), create_default_window(), main(), Arc, EventLoop, Window, WindowResult

### Community 31 - "Community 31"
Cohesion: 1.00
Nodes (3): ConsumerModule (test), GreetingService (test), ProviderModule (test)

### Community 51 - "Community 51"
Cohesion: 0.13
Nodes (13): code:powershell (cd "D:\Projects\Tower of Theos\theos-engine"), code:rust (// explicit import), code:mermaid (flowchart TB), Contribution notes, Design principles, EntityId (canonical decision), Open decisions / TODO, Planned public surface (+5 more)

### Community 52 - "Community 52"
Cohesion: 0.15
Nodes (11): AC traceability, Changes, Confirmed decisions (vs. spec wording), Goal, Other deltas vs. spec wording, Out of scope (deferred), Risks, Tests (`theos-core/src/kernel.rs` `mod tests`) (+3 more)

### Community 53 - "Community 53"
Cohesion: 0.12
Nodes (23): CloseToExitModule, make_initialised_close_module(), tick_ignores_keypress_events(), tick_ignores_resized_events(), tick_is_noop_before_init(), tick_requests_shutdown_on_close_requested(), tick_with_empty_receiver_is_idempotent(), Arc (+15 more)

### Community 56 - "Community 56"
Cohesion: 0.20
Nodes (21): approx(), assemble(), body(), efficiency_matches_manual_eta_on_constructive_graph(), efficiency_on_zero_energy_graph_is_well_defined(), EnvResistance, full_transfer_graph_efficiency_near_one(), graph_energy_matches_manual_sum_over_edges() (+13 more)

### Community 57 - "Community 57"
Cohesion: 0.15
Nodes (17): Arena, Arena<T>, default_arena_is_empty(), get_mut_allows_in_place_update(), get_returns_none_for_stale_handle_after_remove(), insert_reuses_freed_slot_with_bumped_generation(), insert_then_get_returns_the_value(), len_and_is_empty_track_live_values() (+9 more)

### Community 58 - "Community 58"
Cohesion: 0.08
Nodes (23): Acceptance for plan completion, Baseline (2026-06-08, post-THEOS-17 log edits), C1 — EntityId (theos-shared/src/types.rs), C2 — FrameTimer default (theos-core/src/frame_timer.rs), C3 — WindowEvent remaining branches (theos-shared/src/window_event.rs), Coverage forecast, Defer list *(documented, will not be unit-tested)*, File-level highlights (+15 more)

### Community 59 - "Community 59"
Cohesion: 0.22
Nodes (13): clone_shares_state_with_original(), default_equals_new(), handle_flip_is_visible_to_typed_api(), handle_shares_state_with_typed_api(), new_starts_unrequested(), request_shutdown_is_idempotent(), request_shutdown_sets_flag(), ShutdownRequester (+5 more)

### Community 60 - "Community 60"
Cohesion: 0.15
Nodes (12): WindowModule, Arc, Box, EngineModule, Error, KernelContext, ModuleDependency, Result (+4 more)

### Community 61 - "Community 61"
Cohesion: 0.12
Nodes (15): Acceptance-criteria traceability, Confirmed decisions (locked, see concept page), Frame timer (`theos-core::frame_timer`), FrameTimer — covered via existing kernel tests, Kernel — automated unit tests, Kernel (`theos-core`), Launcher (`theos-launcher`), Out of scope (deferred) (+7 more)

### Community 62 - "Community 62"
Cohesion: 0.25
Nodes (12): Component, ComponentValue, T, WorldAccess, WorldAccessExt, Any, Box, Clone (+4 more)

### Community 63 - "Community 63"
Cohesion: 0.20
Nodes (10): EventLoopError, FrameTimer, TheosAppHandler, Arc, AtomicBool, EventLoop, Result, Self (+2 more)

### Community 64 - "Community 64"
Cohesion: 0.15
Nodes (12): Architecture sub-question (decide before AC2), Locked decisions, Option A — `ShutdownRequester` service in `KernelContext`, Option B — `ShutdownRequested` event on the bus, Out of scope (deferred — match the Jira), Remaining implementation work (logic, not scaffolding), Summary, THEOS-16: Input Event Propagation (+4 more)

### Community 65 - "Community 65"
Cohesion: 0.17
Nodes (11): Build and checks, Crate dependency graph (overview), Design principles, Entry points and apps, Foundation, Kernel and subsystems, Notes, Plugin and scripting boundaries (+3 more)

### Community 66 - "Community 66"
Cohesion: 0.29
Nodes (7): Deref, WindowHandle, Target, Arc, Self, TheosService, Window

### Community 67 - "Community 67"
Cohesion: 0.31
Nodes (7): default_returns_first_frame_timer(), FrameTimer, second_tick_returns_real_delta(), Default, Instant, Option, Self

### Community 68 - "Community 68"
Cohesion: 0.22
Nodes (4): KeyCode, NativeKeyCode, WindowEvent, Event

### Community 69 - "Community 69"
Cohesion: 0.22
Nodes (8): Coverage targets, graphify, Manual smoke, Plans, Pre-commit checks, Quality gates, Test scaffolding pattern, Theos-Engine

### Community 70 - "Community 70"
Cohesion: 0.22
Nodes (8): Acceptance-criteria gap table, Crate wiring (done), Deferred / out of scope, Locked decisions, Related, Resolved design decisions (were open during design), Test execution / CI, THEOS-147 — [seam] ECS World access trait + in-memory stub + AI components

### Community 71 - "Community 71"
Cohesion: 0.22
Nodes (8): Acceptance criteria verification, Companion artifact, Files changed, Locked decisions, Operational notes for future readers, Shape of the work, Summary, THEOS-17: Integration Verification

### Community 72 - "Community 72"
Cohesion: 0.33
Nodes (5): Build, Platform requirements, Public API, Testing, theos-window

### Community 73 - "Community 73"
Cohesion: 0.60
Nodes (4): AiBrain, PendingAiResponse, Component, String

### Community 74 - "Community 74"
Cohesion: 0.67
Nodes (3): full_lifecycle_emits_expected_log_sequence(), strip_ansi(), String

### Community 78 - "Community 78"
Cohesion: 0.09
Nodes (20): ElementKind, Attunement, Chakra, chakra_state_round_trips_through_json(), Chakras, chakras_default_is_unattuned_and_balanced(), ChakraState, from_array_preserves_canonical_order() (+12 more)

### Community 79 - "Community 79"
Cohesion: 0.14
Nodes (9): element_field_round_trips_through_json(), ElementField, ElementKind, uniform_sets_every_element(), Index, IndexMut, Item, Iterator (+1 more)

### Community 80 - "Community 80"
Cohesion: 0.12
Nodes (19): KeyCode, NativeKeyCode, PhysicalKey, translator_constructs_with_event_bus_handle(), WindowEventTranslator, assert_maps(), translate_keycode_maps_all_digits(), translate_keycode_maps_all_letters() (+11 more)

### Community 81 - "Community 81"
Cohesion: 0.29
Nodes (6): Acceptance-criteria gap table, Deferred, Locked decisions, Resolved decisions (chakra & element model), Test execution, THEOS-148 — [M1][magic] theos-magic crate scaffold & core HarmonicToken seam

### Community 82 - "Community 82"
Cohesion: 0.21
Nodes (21): approx(), band_pass_response(), band_pass_response_attenuates_off_center(), band_pass_response_is_one_at_center(), band_pass_response_never_exceeds_one(), clamp_tau_pins_huge_duration_to_max(), clamp_tau_pins_zero_duration_to_min(), clamp_tau_seconds() (+13 more)

### Community 84 - "Community 84"
Cohesion: 0.08
Nodes (54): Chakra, approx(), assemble(), assemble_default_uses_the_default_wave_speed(), body(), cast_time_equals_critical_path_delay_when_no_transfers_present(), cast_time_on_acyclic_graph_matches_manual_calc(), cast_time_on_cyclic_graph_matches_manual_calc() (+46 more)

### Community 85 - "Community 85"
Cohesion: 0.10
Nodes (20): Arena, CsrAdjacency, Edge, CsrAdjacency, Edge, neighbors_is_empty_for_a_sink_node(), neighbors_returns_a_nodes_outgoing_edges(), offsets_length_is_node_count_plus_one() (+12 more)

### Community 86 - "Community 86"
Cohesion: 0.17
Nodes (11): AC → file gap table, Chakra layer (`chakra.rs`), Coverage target, Deferred / out of scope, Graph layer (`graph/`), Locked decisions (design session 2026-06-13), Scaffolding contract, Seam ripple (`interface.rs`, `error.rs`) (+3 more)

### Community 87 - "Community 87"
Cohesion: 0.08
Nodes (20): Add, AddAssign, glam interoperability bridge pattern, mint interoperability bridge pattern, 16-byte SIMD alignment for GPU/SIMD compatibility, Div, DivAssign, f32x4 (+12 more)

### Community 88 - "Community 88"
Cohesion: 0.22
Nodes (10): CompilerContext, CompilerError, HarmonicToken, DummyModality, MagicInterface, CompilerContext, CompilerError, Result (+2 more)

### Community 89 - "Community 89"
Cohesion: 0.25
Nodes (7): Acceptance criteria, Coverage, Deferred / follow-ups, How the scope changed (design journey, 2026-06-14), Implementation map, Locked equations (E12), THEOS-157 — [M2][magic] TemporalFrequency → transfer element (E12)

### Community 90 - "Community 90"
Cohesion: 0.33
Nodes (4): default_tau_clamp_window_is_ordered(), WeaveTuning, Default, Self

### Community 91 - "Community 91"
Cohesion: 0.43
Nodes (6): Chakras, ElementField, CompilerContext, Chakras, EntityId, WeaveTuning

### Community 92 - "Community 92"
Cohesion: 0.33
Nodes (5): AC mapping, Deferred / out of scope, Implementation locus, Locked decisions, THEOS-156 — Cast-time from critical-path propagation delay (E13)

### Community 93 - "Community 93"
Cohesion: 0.12
Nodes (16): Acceptance criteria → coverage, Canon (pinned from *Equations of the Weave*, page 170164225), Cross-story notes, Current code state (`theos-magic/src/graph`), D1 — Where do `kᵢⱼ`, `Lᵢⱼ`, `σ_f` live?  ⚠️ architecture (the ticket's flagged call), D2 — Default values (no token supplies these yet), D3 — `σ_f` (frequency tolerance) source, D4 — Authoring/execution split for `S` (mirror 159 D5)  ⚠️ keep consistent (+8 more)

### Community 94 - "Community 94"
Cohesion: 0.12
Nodes (16): Acceptance criteria → coverage, Canon (pinned from *Shared Formalism: Equations of the Weave*, page 170164225), Current code state (grounding — `theos-magic/src/graph`), D1 — What is the per-edge amplitude `Aᵢⱼ`?  ⚠️ core, D2 — Where do `ρ_med` and `ρ_env` come from? (not in the data model today), D3 — Which nodes are "effectors" for the E10 numerator?  ⚠️ core, D4 — η when `E_cost == 0` (no-injection / zero-energy graph)? (AC3), D5 — Authoring-side (frozen) or execution-side (env-dependent)?  ⚠️ architecture, couples to THEOS-173 (+8 more)

### Community 95 - "Community 95"
Cohesion: 0.12
Nodes (15): Acceptance criteria, Canon state, Current code state, Decision register — **LOCKED** (2026-06-21), DP1 — **What IS `P`?**  ⚠️ worldbuilding — your call (no canon today), DP2 — Aggregation (if `P` derives from chakras), DP3 — `effective_cast_time` seam (scaffolding), DP4 — `P ≤ 0` guard (+7 more)

### Community 96 - "Community 96"
Cohesion: 0.17
Nodes (11): Axis 1 — Frequency `f` (E1): the *material* spectrum — **LOCKED: logarithmic**, Axis 2 — Phase coherence: **Light ↔ Darkness** polarity — **curve: sigmoid (confirm)**, Canon impact (edits this implies), Cultivation & resource economy — DESIGN STUB (own ticket; NOT in the canon prompts), Decisions (LOCKED 2026-06-21 unless noted), Element Cosmology — 2-D (frequency × phase-polarity), Goal, Relationship to sprint stories (+3 more)

### Community 97 - "Community 97"
Cohesion: 0.18
Nodes (10): Acceptance criteria, Decision register — **OPEN** (two coupled worldbuilding calls), Delivery, DN1 — Range `[f_min, f_max]` (the "authored frequency range", Hz), DN2 — Curve (linear vs logarithmic), DN3 — Clamp, Goal (one sentence), Grounded context (from *Core UX: Spell Appearance*, page 177897474, §3–4) (+2 more)

### Community 98 - "Community 98"
Cohesion: 0.33
Nodes (5): Acceptance criteria, Agent prompt — THEOS-170: reconcile E12 (TemporalFrequency) to the implemented transfer-element model, Constraints, Edits, The canonical model (ground truth = the shipped code)

### Community 99 - "Community 99"
Cohesion: 0.40
Nodes (4): Current focus, Sequence (decided 2026-06-21), 🎯 Sprint Planning Summit — Goal, Success criteria

### Community 100 - "Community 100"
Cohesion: 0.16
Nodes (27): approx(), assemble(), body(), coherence_high_for_in_phase_frequency_matched_edge(), coherence_low_for_divergent_frequencies_via_sigma_f(), constructive_graph_is_stable(), cyclic_feedback_runaway_is_volatile(), div() (+19 more)

### Community 101 - "Community 101"
Cohesion: 0.18
Nodes (28): EnvResistance, approx(), body(), CannotChannel, caster(), caster_with_root_power(), effective_cast_time_is_energy_time_when_throughput_bound(), effective_cast_time_is_settling_when_propagation_bound() (+20 more)

### Community 102 - "Community 102"
Cohesion: 0.23
Nodes (18): approx(), body(), gain_differs_with_environment_on_same_graph(), gain_matches_manual_q_for_known_s_and_gamma(), gain_near_one_for_well_damped_graph(), gain_rises_as_stability_margin_thins(), ResonanceRunaway, runaway_when_stability_nonpositive() (+10 more)

### Community 103 - "Community 103"
Cohesion: 0.20
Nodes (9): Acceptance criteria, Canon (E14 B2, landed 2026-06-25), Current code state, Decision register — LOCKED (2026-06-25), Goal (one sentence), Out of scope, Proposed shape (scaffolding only), Test scenarios (names only) (+1 more)

### Community 104 - "Community 104"
Cohesion: 0.67
Nodes (5): bench_mat3_mul(), bench_mat4_mul(), bench_vec3_cross(), bench_vec3_dot(), Criterion

### Community 107 - "Community 107"
Cohesion: 0.22
Nodes (8): Acceptance criteria, Canon (E15, Equations of the Weave), Decision register — LOCKED (2026-06-25), Goal (one sentence), Out of scope, Proposed shape (scaffolding only), Test scenarios (names only), THEOS-176 — [magic] E15 resonance gain (Q = 1/Sᵞ)

## Knowledge Gaps
- **420 isolated node(s):** `String`, `ModuleResult`, `Vec`, `Tag`, `Result` (+415 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **19 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `BroadcastEventBus` connect `Community 4` to `Community 80`?**
  _High betweenness centrality (0.013) - this node is a cross-community bridge._
- **Why does `KeyCode` connect `Community 80` to `Community 23`?**
  _High betweenness centrality (0.013) - this node is a cross-community bridge._
- **What connects `String`, `ModuleResult`, `Vec` to the rest of the system?**
  _422 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Community 0` be split into smaller, more focused modules?**
  _Cohesion score 0.07769423558897243 - nodes in this community are weakly interconnected._
- **Should `Community 1` be split into smaller, more focused modules?**
  _Cohesion score 0.08583959899749373 - nodes in this community are weakly interconnected._
- **Should `Community 2` be split into smaller, more focused modules?**
  _Cohesion score 0.06693803708729082 - nodes in this community are weakly interconnected._
- **Should `Community 3` be split into smaller, more focused modules?**
  _Cohesion score 0.07756813417190776 - nodes in this community are weakly interconnected._