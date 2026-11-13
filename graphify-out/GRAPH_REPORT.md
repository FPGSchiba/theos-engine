# Graph Report - npc-llm-poc  (2026-11-13)

## Corpus Check
- 285 files · ~137,688 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 2566 nodes · 5180 edges · 268 communities (232 shown, 36 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 100 edges (avg confidence: 0.84)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `22f26159`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- ConsumerModule
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
- Self
- TheosFormatter
- THEOS-158 — [magic] Injection + E4 interference amplitude (A_j)
- .init
- theos-engine
- key_translate.rs
- KernelContext
- theos-launcher main
- npc-poc/src/main.rs
- .record_debug
- Strict DAG No-Circular-Dependency Crate Layout
- benches/event_bus.rs
- .sender_for_erased
- EventBus
- ModuleDependency
- chakra.rs
- THEOS-181 — [magic] E16c construction transient + per-chakra throughput meter / overdraw
- RecordId
- CallLog
- transient.rs
- retrieval.rs
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
- reply.rs
- Engine Module three-phase lifecycle (init, tick, shutdown)
- Service Locator pattern for engine subsystem registry
- Chakras
- ElementField
- worker.rs
- THEOS-148 — [M1][magic] theos-magic crate scaffold & core HarmonicToken seam
- 16-byte SIMD alignment for GPU/SIMD compatibility
- npc-poc
- assembler.rs
- theos-assets
- Locked decisions (design session 2026-06-13)
- Vec4
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
- theos-ai/src/testkit.rs
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
- report.rs
- driver.rs
- run.rs
- score.rs
- theos-ai/src/context.rs
- ModelContext
- convert.rs
- BackendError
- WovenToken
- NPC LLM proof of concept — design
- LlamaDecoder
- .assemble
- embedder.rs
- File structure
- plane.rs
- generator.rs
- scenarios.rs
- kernel_integration.rs
- lore_poc.rs
- TheosAppHandler
- Vec3
- ray.rs
- MemoryProbe
- mini
- TimingModule
- spike.rs
- LlamaError
- lore-poc fixture
- KernelResult
- Clock

## God Nodes (most connected - your core abstractions)
1. `Vec3` - 92 edges
2. `RecordId` - 42 edges
3. `TheosKernel` - 38 edges
4. `Vec4` - 38 edges
5. `mini()` - 34 edges
6. `Vec2` - 34 edges
7. `Chakras` - 32 edges
8. `assemble()` - 32 edges
9. `WovenToken` - 30 edges
10. `Chakra` - 28 edges

## Surprising Connections (you probably didn't know these)
- `Type-Erased EventBus via ChannelHandle adapter` --semantically_similar_to--> `Service Locator encapsulated in KernelContext`  [INFERRED] [semantically similar]
  theos-core/src/event.rs → plans/THEOS-86.md
- `PhysicsEvent` --semantically_similar_to--> `GameEvent`  [INFERRED] [semantically similar]
  theos-physics/src/event.rs → theos-core/src/event.rs
- `THEOS-86 Microkernel Bootstrap Plan` --references--> `CrossbeamEventBus`  [EXTRACTED]
  plans/THEOS-86.md → theos-core/src/event.rs
- `assemble()` --calls--> `visible()`  [INFERRED]
  theos-ai/src/context.rs → plugins/llm-poc/npc-poc/src/score.rs
- `visible_matches_golden_for_all_eight_speakers()` --calls--> `visible()`  [INFERRED]
  theos-ai/tests/lore_poc.rs → plugins/llm-poc/npc-poc/src/score.rs

## Import Cycles
- 2-file cycle: `theos-ai/src/driver.rs -> theos-ai/src/reply.rs -> theos-ai/src/driver.rs`
- 2-file cycle: `plugins/llm-poc/npc-poc/src/report.rs -> plugins/llm-poc/npc-poc/src/run.rs -> plugins/llm-poc/npc-poc/src/report.rs`

## Hyperedges (group relationships)
- **Unsafe Code Audit Triad (Clippy, Forbid, Geiger)** — github_workflows_check_lint_clippy_unsafe_policy_job, github_workflows_check_lint_forbid_unsafe_verification, github_workflows_check_lint_cargo_geiger [EXTRACTED 0.95]
- **Cross-Module Service Discovery via KernelContext: ProviderModule registers, ConsumerModule retrieves** — theos_core_tests_kernel_integration_providermodule, theos_core_tests_kernel_integration_consumermodule, theos_core_tests_kernel_integration_greetingservice [EXTRACTED 1.00]
- **Type-Erased EventBus: CrossbeamEventBus stores ChannelHandle<TypedChannel<E>> keyed by TypeId** — src_lib_crossbeameventbus, src_lib_channelhandle, theos_core_src_event_typedchannel_e [EXTRACTED 1.00]
- **Kernel Lifecycle: Register → DependencyResolver sort → init → fixed-step loop → shutdown** — theos_core_src_kernel_theoskernel, theos_core_src_dependency_resolver_dependencyresolver, concept_fixed_step_loop [EXTRACTED 1.00]
- **Release Artifact Production (Launcher, Game, Editor)** — github_workflows_release_packaging_theos_launcher_artifact, github_workflows_release_packaging_theos_game_artifact, github_workflows_release_packaging_theos_editor_artifact [EXTRACTED 1.00]
- **CI Quality Gate Pipeline (Check, Lint, Test)** — github_workflows_check_lint_check_lint_workflow, github_workflows_test_test_workflow, github_workflows_check_lint_clippy_unsafe_policy_job [INFERRED 0.85]

## Communities (268 total, 36 thin omitted)

### Community 0 - "ConsumerModule"
Cohesion: 0.13
Nodes (8): ConsumerModule, CountingModule<Tag>, GreetingService, ProviderModule, Mutex, Option, String, TypeId

### Community 1 - ".sender_for_erased"
Cohesion: 0.44
Nodes (6): Any, Box, EventBusResult, Send, TypeId, StubBus

### Community 2 - "theos-core/src/event.rs"
Cohesion: 0.10
Nodes (9): BroadcastEventBus, Debug, E, Formatter, HashMap, Result, RwLock, Sender (+1 more)

### Community 3 - "dependency_resolver.rs"
Cohesion: 0.07
Nodes (30): ModuleResult, five_module_mixed_graph_satisfies_all_dependencies(), linear_chain_resolves_in_dependency_order(), module(), module_with_deps(), Box, Error, PhantomData (+22 more)

### Community 4 - "TheosKernel"
Cohesion: 0.17
Nodes (12): Fixed-Step Game Loop (60 Hz sleep+spin hybrid), Kahn's Topological Sort for Module Init Order, Service Locator encapsulated in KernelContext, THEOS-86 Microkernel Bootstrap Plan, DependencyResolver, event_bus_handle_returns_clone_of_kernel_bus(), Arc, AtomicU32 (+4 more)

### Community 5 - "mat.rs"
Cohesion: 0.11
Nodes (20): glam::Mat3, glam::Mat4, Mat3, Mat4, From, Mul, Output, Self (+12 more)

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
Cohesion: 0.13
Nodes (31): calling_init_twice_returns_already_initialized(), default_constructs_kernel_with_no_modules_and_uninitialised_flag(), init_calls_module_init_in_dependency_sorted_order(), init_propagates_dependency_resolver_error_without_running_any_module(), init_stops_at_first_failing_module_and_skips_later_modules(), init_with_no_modules_succeeds_and_marks_kernel_initialised(), registering_duplicate_name_errors_and_leaves_buffer_unchanged(), registering_three_distinct_modules_fills_staging_buffer() (+23 more)

### Community 10 - ".new"
Cohesion: 0.16
Nodes (21): test_glam_vec2_roundtrip(), test_glam_vec3_and_vec3a_produce_same_components(), test_glam_vec3_roundtrip(), test_glam_vec3a_roundtrip(), test_glam_vec4_roundtrip(), test_vec2_constructors_and_dot(), test_vec2_dot_antiparallel_is_negative(), test_vec2_dot_orthogonal_is_zero() (+13 more)

### Community 11 - "CrossbeamEventBus"
Cohesion: 0.47
Nodes (6): Type-Erased EventBus via ChannelHandle adapter, ChannelHandle (private trait), CrossbeamEventBus, allocation_check_typed_event_bus (alloc test), bench_send_then_drain_typed (Criterion benchmark), GameEvent

### Community 12 - "Release Packaging Job"
Cohesion: 0.09
Nodes (23): Cargo Plugins Workspace Dependency Updates (theos-plugin-deps), Cargo Workspace Dependency Updates (theos-engine-deps), Dependabot Configuration, GitHub Actions Dependency Updates, cargo-geiger Unsafe Audit Tool, Cargo Check Job, Check and Lint Workflow, Clippy and Unsafe Policy Job (+15 more)

### Community 13 - "upkeep.rs"
Cohesion: 0.22
Nodes (27): approx(), assemble(), body(), occ(), Option, Vec, same_construct_two_environments_differ_only_in_upkeep_rate(), spatial() (+19 more)

### Community 14 - "EntityId"
Cohesion: 0.06
Nodes (45): AtomicU64, EntityMap, AiBrain, PendingAiResponse, String, component_ids_lists_inserted_component_types(), contains_is_true_for_spawned_entity(), despawn_removes_entity_and_its_components() (+37 more)

### Community 15 - "THEOS-86: Microkernel Core Bootstrap — Entry Point, Thread Pool & Kernel Initialization"
Cohesion: 0.04
Nodes (44): 1.1 Define `EngineModule` Trait (in `theos-shared`), 1.2 Define `EventBus` Trait (in `theos-shared`), 1.3 Create `ServiceLocator` Struct (in `theos-shared`), 1.4 Stub Out `TheosKernel` Struct (in `theos-core`), 1.5 Define `AiWorkerHandle` Struct (in `theos-core`), 1.6 Define `AiCommandQueue` Struct (in `theos-core`), 1.7 Design `ModuleRegistry` (Logical Component), 2.1 Logging Initialization (+36 more)

### Community 16 - "ServiceLocator"
Cohesion: 0.17
Nodes (14): ServiceLocatorResult, register_and_get_service(), Any, Arc, HashMap, Option, RwLock, Send (+6 more)

### Community 18 - "Self"
Cohesion: 0.14
Nodes (4): Output, Self, test_simd_interop(), wide::f32x4

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

### Community 23 - "key_translate.rs"
Cohesion: 0.12
Nodes (14): PhysicalKey, KeyCode, NativeKeyCode, WindowEvent, assert_maps(), translate_keycode_maps_all_digits(), translate_keycode_maps_all_letters(), translate_keycode_maps_editing_keys() (+6 more)

### Community 24 - "KernelContext"
Cohesion: 0.20
Nodes (13): KernelContextResult, event_bus_hands_back_the_same_shared_handle(), get_unregistered_service_returns_service_not_found(), KernelContext, make_context(), register_overwrites_previous_service_of_same_type(), register_then_get_returns_the_service(), Arc (+5 more)

### Community 25 - "theos-launcher main"
Cohesion: 1.00
Nodes (3): theos-launcher main, launcher log filtering test: defaults_to_info_when_rust_log_missing, launcher log filtering test: invalid_rust_log_does_not_crash_and_falls_back

### Community 26 - "npc-poc/src/main.rs"
Cohesion: 0.11
Nodes (26): Cli, Command, default_fixture(), export(), main(), Option, Path, PathBuf (+18 more)

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
Cohesion: 0.34
Nodes (8): Any, Box, EventBusResult, Option, Send, Sync, TypeId, TypedChannel<E>

### Community 31 - "EventBus"
Cohesion: 0.18
Nodes (14): BroadcastBusExt, T, Test2Event, TestEvent, PhysicsEvent, ChannelHandle, Event, EventBus (+6 more)

### Community 32 - "ModuleDependency"
Cohesion: 0.18
Nodes (7): FailingInitModule, RecordingModule<Tag>, TypeId, ModuleDependency, Self, TypeId, VersionReq

### Community 33 - "chakra.rs"
Cohesion: 0.20
Nodes (9): Attunement, chakra_state_round_trips_through_json(), chakras_default_is_unattuned_and_balanced(), from_array_preserves_canonical_order(), indexing_and_iteration_observe_the_same_state(), iter_mut_allows_mutation_of_every_point(), Polarity, Default (+1 more)

### Community 34 - "THEOS-181 — [magic] E16c construction transient + per-chakra throughput meter / overdraw"
Cohesion: 0.15
Nodes (12): Acceptance criteria (from Jira), Canon (Equations of the Weave, E16c — pinned + the gap this story fills), Canon writeback (E16c has a genuine gap — for the canon-authority agent), Coverage note, Current code state (all inputs already exist), Decision register — LOCKED (2026-08-29, guided session), Goal (one sentence), Implementation notes (2026-08-29, guided) (+4 more)

### Community 35 - "RecordId"
Cohesion: 0.08
Nodes (36): Into, count(), belief(), ids(), Item, Iterator, Vec, Visible (+28 more)

### Community 36 - "CallLog"
Cohesion: 0.31
Nodes (6): CallLog, RecordingModule, Mutex, PhantomData, Tag, Vec

### Community 37 - "transient.rs"
Cohesion: 0.15
Nodes (46): Chakra, approx(), assemble(), body(), Construct, construct_ids_are_unique_across_spawns(), ConstructId, Lane (+38 more)

### Community 38 - "retrieval.rs"
Cohesion: 0.12
Nodes (32): cosine(), Embedder, EmbedError, EmbedInput, String, build_embeds_only_beliefs(), build_errors_when_embedder_returns_vectors_of_differing_dimensions(), build_errors_when_embedder_returns_wrong_vector_count() (+24 more)

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
Cohesion: 0.06
Nodes (47): CloseToExitModule, make_initialised_close_module(), Arc, Box, Default, Error, Option, Receiver (+39 more)

### Community 56 - "WeaveTuning"
Cohesion: 0.07
Nodes (56): approx(), assemble(), body(), efficiency_matches_manual_eta_on_constructive_graph(), efficiency_on_zero_energy_graph_is_well_defined(), EnvResistance, full_transfer_graph_efficiency_near_one(), graph_energy_matches_manual_sum_over_edges() (+48 more)

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
Cohesion: 0.14
Nodes (14): default_returns_first_frame_timer(), FrameTimer, Default, Instant, Option, Self, second_tick_returns_real_delta(), get_engine_version() (+6 more)

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

### Community 74 - "reply.rs"
Cohesion: 0.10
Nodes (41): embedder(), embedder_ranks_related_lore_above_unrelated(), embedder_returns_unit_vectors_of_dimension_1024(), generator(), generator_emits_thinking_pieces_then_answer_when_thinking_on(), generator_reply_parses_with_theos_ai_parse_reply(), gpu(), ids() (+33 more)

### Community 78 - "Chakras"
Cohesion: 0.15
Nodes (8): Chakras, ChakraState, named_accessors_reach_every_point(), Index, IndexMut, Item, Iterator, Option

### Community 79 - "ElementField"
Cohesion: 0.12
Nodes (10): CompilerContext, element_field_round_trips_through_json(), ElementField, ElementKind, Index, IndexMut, Item, Iterator (+2 more)

### Community 80 - "worker.rs"
Cohesion: 0.11
Nodes (32): AtomicUsize, Completed, Drop, F, JoinHandle, counting_worker(), CountingBackend, drop_discards_queued_requests() (+24 more)

### Community 81 - "THEOS-148 — [M1][magic] theos-magic crate scaffold & core HarmonicToken seam"
Cohesion: 0.29
Nodes (6): Acceptance-criteria gap table, Deferred, Locked decisions, Resolved decisions (chakra & element model), Test execution, THEOS-148 — [M1][magic] theos-magic crate scaffold & core HarmonicToken seam

### Community 83 - "npc-poc"
Cohesion: 1.00
Nodes (3): npc-poc, theos-ai, theos-ai-llama

### Community 84 - "assembler.rs"
Cohesion: 0.16
Nodes (34): approx(), assemble(), body(), cast_time_equals_critical_path_delay_when_no_transfers_present(), cast_time_on_acyclic_graph_matches_manual_calc(), cast_time_on_cyclic_graph_matches_manual_calc(), chooses_lowest_index_chakra_as_injection_root(), convergent_node_amplitude_adds_when_in_phase() (+26 more)

### Community 86 - "Locked decisions (design session 2026-06-13)"
Cohesion: 0.17
Nodes (11): AC → file gap table, Chakra layer (`chakra.rs`), Coverage target, Deferred / out of scope, Graph layer (`graph/`), Locked decisions (design session 2026-06-13), Scaffolding contract, Seam ripple (`interface.rs`, `error.rs`) (+3 more)

### Community 87 - "Vec4"
Cohesion: 0.08
Nodes (17): Add, AddAssign, Div, DivAssign, f32x4, Sub, SubAssign, glam::Vec2 (+9 more)

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
Cohesion: 0.22
Nodes (22): approx(), body(), CannotChannel, caster(), caster_with_root_power(), effective_cast_time_is_energy_time_when_throughput_bound(), effective_cast_time_is_settling_when_propagation_bound(), frozen_cast_time_unchanged_by_floor() (+14 more)

### Community 102 - "theos-ai/src/testkit.rs"
Cohesion: 0.11
Nodes (23): fake_embedder_is_deterministic(), fake_embedder_scores_shared_words_higher(), FakeClock, FakeEmbedder, FakeProbe, lock(), one(), Arc (+15 more)

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

### Community 120 - "report.rs"
Cohesion: 0.14
Nodes (37): answers_mark_hand_read_scenarios_and_missing_replies(), behaviour_label(), csv_quotes_replies_with_commas_and_newlines(), failed_row(), failed_rows_show_their_error_in_every_table_and_count_as_failures(), gib(), header(), join_ids() (+29 more)

### Community 121 - "driver.rs"
Cohesion: 0.16
Nodes (33): answer_counts_thinking_and_answer_tokens_separately_and_parses_only_answer(), answer_errors_on_unknown_speaker(), answer_keeps_highest_vram_sample(), answer_measures_time_to_first_token_total_and_tokens_per_second(), answer_passes_prompt_schema_and_thinking_to_backend(), answer_pieces(), answer_records_context_ids_from_retrieval(), answer_returns_parsed_reply_and_cited_ids() (+25 more)

### Community 122 - "run.rs"
Cohesion: 0.14
Nodes (32): a_failing_request_becomes_a_failed_row_and_the_run_continues(), a_worker_that_dies_mid_run_returns_the_rows_so_far_and_a_fatal_error(), defects(), defects_name_rows_with_a_leak_or_forbidden_record(), fixture_matches_committed_snapshots(), flaky_worker(), gpu_name(), LoadInfo (+24 more)

### Community 123 - "score.rs"
Cohesion: 0.15
Nodes (28): an_unusable_reply_fails_grounding_and_has_no_words(), check(), Checks, deadline_fails_on_deadline_stop_or_slow_total(), forbidden_fails_when_a_forbid_id_is_in_the_context(), forbidden_words(), found(), grounded() (+20 more)

### Community 124 - "theos-ai/src/context.rs"
Cohesion: 0.17
Nodes (26): assemble(), assemble_drops_lore_ids_the_speaker_may_not_see(), assemble_includes_persona_bodies_by_default(), assemble_lists_held_under_what_you_know_with_bracketed_ids(), assemble_lists_rejected_under_what_you_reject(), assemble_names_speaker_role_and_lens(), assemble_omits_persona_bodies_when_disabled(), assemble_puts_question_in_user() (+18 more)

### Community 125 - "ModelContext"
Cohesion: 0.14
Nodes (15): EmbeddingsSeqError, LlamaContext, LlamaContextParams, LlamaRuntime, ModelContext, Arc, DecodeError, LlamaBackend (+7 more)

### Community 126 - "convert.rs"
Cohesion: 0.20
Nodes (24): b942_voice_has_saying_without_header(), binder_speaker_has_two_layers_and_throat_kin_lens(), body_text(), convert_lore(), convert_speaker(), fixture_converts_to_beliefs_truths_clues_topics_and_eight_speakers(), holder_kind(), list() (+16 more)

### Community 127 - "BackendError"
Cohesion: 0.14
Nodes (18): Flaky, Box, Backend, BackendError, Decoder, GenerationRequest, Phase, Piece (+10 more)

### Community 128 - "WovenToken"
Cohesion: 0.11
Nodes (14): kinematic(), kinematic_vector_into_an_effector_is_allowed(), body(), Vec, spatial(), DummyModality, MagicInterface, Result (+6 more)

### Community 129 - "NPC LLM proof of concept — design"
Cohesion: 0.08
Nodes (23): 10. Order of work, 11. Left to the builder, 12. Calls made by the designer (not discussed with the owner), 1. Intent, 2. Goals, non-goals, anti-goals, 3. Fixed decisions (settled before this session — do not reopen), 4. Decided in this session, 5. Findings from recon (2026-11-13) (+15 more)

### Community 130 - "LlamaDecoder"
Cohesion: 0.15
Nodes (16): LlamaChatTemplate, Matcher, ParserFactory, LlamaDecoder, LlamaGenerator, Box, LlamaBatch, LlamaSampler (+8 more)

### Community 131 - ".assemble"
Cohesion: 0.11
Nodes (13): CompilerError, String, assemble_default_uses_the_default_wave_speed(), GraphAssembler, Result, TensionGraph, injection_attenuation(), injection_attenuation_composes_multiplicatively() (+5 more)

### Community 132 - "embedder.rs"
Cohesion: 0.16
Nodes (18): Cow, document_text_is_embedded_as_given(), ensure_ends_with(), ensure_ends_with_appends_missing_end_token_once(), ensure_ends_with_handles_empty_sequence(), input_text(), l2_normalize(), l2_normalize_leaves_zero_vector_zero() (+10 more)

### Community 133 - "File structure"
Cohesion: 0.11
Nodes (18): Deferred, File structure, Global Constraints, NPC LLM Proof of Concept Implementation Plan, Review Focus, Spike results, Task 0: Branch setup (controller), Task 10: Harness `run` — scoring and results tables (+10 more)

### Community 134 - "plane.rs"
Cohesion: 0.26
Nodes (15): assert_vec3_eq(), Plane, Option, Self, test_intersects_ray_from_above_hits_origin(), test_intersects_ray_from_below_hits_plane(), test_intersects_ray_offset_origin(), test_intersects_ray_parallel_returns_none() (+7 more)

### Community 135 - "generator.rs"
Cohesion: 0.20
Nodes (12): decode_error(), ends_thinking(), ModelEntry, ModelProfile, ModelsFile, profile(), render_prompt(), render_prompt_appends_thinking_off_prefix() (+4 more)

### Community 136 - "scenarios.rs"
Cohesion: 0.24
Nodes (16): scenario(), Behaviour, fixture(), load_scenarios(), load_visible(), BTreeMap, Path, PathBuf (+8 more)

### Community 137 - "kernel_integration.rs"
Cohesion: 0.24
Nodes (12): kernel_runs_init_tick_shutdown_lifecycle(), module_retrieves_service_registered_by_its_dependency(), reports_frame_timing_accuracy(), reports_frame_timing_multirun(), Box, Error, Result, Send (+4 more)

### Community 138 - "lore_poc.rs"
Cohesion: 0.22
Nodes (13): Adversarial, fixture(), Golden, no_forbidden_record_reaches_any_prompt(), Result, Scenario, String, Vec (+5 more)

### Community 139 - "TheosAppHandler"
Cohesion: 0.18
Nodes (11): Arc, AtomicBool, EventLoop, EventLoopError, Result, Self, TheosAppHandler, Arc (+3 more)

### Community 140 - "Vec3"
Cohesion: 0.23
Nodes (10): f32, glam::Vec3, glam::Vec3A, mint::Vector3<f32>, From, Mul, Vec3, Vec3Swizzle (+2 more)

### Community 141 - "ray.rs"
Cohesion: 0.33
Nodes (11): assert_vec3_eq(), Ray, Self, test_at_known_t_value(), test_at_negative_t_goes_behind_origin(), test_at_one_returns_origin_plus_direction(), test_at_zero_returns_origin(), test_new_accepts_axis_aligned_directions() (+3 more)

### Community 142 - "MemoryProbe"
Cohesion: 0.21
Nodes (6): IDXGIAdapter3, DxgiProbe, Option, MemoryProbe, NoProbe, Option

### Community 143 - "mini"
Cohesion: 0.39
Nodes (11): visible(), visible_excludes_a_rejected_belief_listed_in_unaware_of(), visible_excludes_unaware_of_ids(), visible_held_excludes_beliefs_of_other_holders(), visible_held_includes_beliefs_held_by_any_layer(), visible_ignores_rejects_ids_missing_from_corpus(), visible_lists_a_belief_both_held_and_rejected_only_as_rejected(), visible_never_includes_truths_clues_or_topics() (+3 more)

### Community 144 - "TimingModule"
Cohesion: 0.26
Nodes (10): CountingModule, make_counting_module(), Arc, AtomicBool, AtomicU32, Instant, PhantomData, Tag (+2 more)

### Community 145 - "spike.rs"
Cohesion: 0.31
Nodes (10): dxgi_usage(), generate(), main(), print_dxgi(), LlamaBackend, LlamaModel, LlamaSampler, Option (+2 more)

### Community 146 - "LlamaError"
Cohesion: 0.24
Nodes (8): models_toml_parses_profile(), models_toml_rejects_missing_sampling(), Arc, Path, Self, LlamaError, PathBuf, String

### Community 147 - "lore-poc fixture"
Cohesion: 0.20
Nodes (9): Checks run, Layout, lore-poc fixture, Notes on the sources, Scenario fields, Scoring, The speaker rule, What the fixture adds (+1 more)

### Community 148 - "KernelResult"
Cohesion: 0.38
Nodes (3): KernelResult, Option, step_before_init_returns_uninitialized_error()

### Community 149 - "Clock"
Cohesion: 0.53
Nodes (4): Clock, Instant, system_clock_never_goes_backwards(), SystemClock

## Knowledge Gaps
- **367 isolated node(s):** `theos-audio`, `theos-assets`, `TagA`, `TagB`, `TagC` (+362 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **36 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `EntityId` connect `EntityId` to `KernelError`, `ElementField`?**
  _High betweenness centrality (0.289) - this node is a cross-community bridge._
- **Why does `TheosKernel` connect `TheosKernel` to `theos-core/src/event.rs`, `dependency_resolver.rs`, `CallLog`, `KernelError`, `kernel.rs`, `CrossbeamEventBus`, `TheosAppHandler`, `TimingModule`, `TheosFormatter`, `KernelResult`, `.init`, `CloseToExitModule`?**
  _High betweenness centrality (0.182) - this node is a cross-community bridge._
- **What connects `theos-audio`, `theos-assets`, `TagA` to the rest of the system?**
  _367 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `ConsumerModule` be split into smaller, more focused modules?**
  _Cohesion score 0.13071895424836602 - nodes in this community are weakly interconnected._
- **Should `theos-core/src/event.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.09881422924901186 - nodes in this community are weakly interconnected._
- **Should `dependency_resolver.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.07307692307692308 - nodes in this community are weakly interconnected._
- **Should `mat.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.1140819964349376 - nodes in this community are weakly interconnected._