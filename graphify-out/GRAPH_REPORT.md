# Graph Report - npc-llm-poc  (2026-11-17)

## Corpus Check
- 378 files · ~316,736 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 3608 nodes · 8302 edges · 303 communities (263 shown, 40 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 200 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `06d0e791`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- run_cli
- CloseToExitModule
- theos-core/src/event.rs
- dependency_resolver.rs
- TheosKernel
- mat.rs
- KernelError
- quat.rs
- aabb.rs
- kernel.rs
- .new
- prompt.rs
- Release Packaging Job
- upkeep.rs
- EntityId
- THEOS-86: Microkernel Core Bootstrap — Entry Point, Thread Pool & Kernel Initialization
- ServiceLocator
- bt-poc/src/run.rs
- Self
- worker.rs
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
- BroadcastEventBus
- EventBus
- ModuleDependency
- knn.rs
- THEOS-181 — [magic] E16c construction transient + per-chakra throughput meter / overdraw
- .beliefs
- CallLog
- transient.rs
- Tag
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
- EscapeToQuitModule
- theos-scripting/src/lib.rs
- LlamaDecoder
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
- theos-ai-llama/tests/real_model.rs
- Engine Module three-phase lifecycle (init, tick, shutdown)
- Service Locator pattern for engine subsystem registry
- bt_spike.rs
- ElementField
- npc-poc/src/run.rs
- THEOS-148 — [M1][magic] theos-magic crate scaffold & core HarmonicToken seam
- 16-byte SIMD alignment for GPU/SIMD compatibility
- theos-ai
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
- driver.rs
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
- npc-poc/src/report.rs
- spawn_worker
- gain.rs
- npc-poc/src/score.rs
- bt-poc/src/report.rs
- tune.rs
- ModelContext
- Npc
- WovenToken
- NPC LLM proof of concept — design
- TheosFormatter
- Review Focus
- rig
- File structure
- NPC guardrails: content moderation and the input channel — design
- generator.rs
- bt-poc/src/score.rs
- check.py
- ShutdownRequester
- Chakras
- Vec3
- ray.rs
- embedder.rs
- LlamaGenerator
- bt-poc/src/inject.rs
- spike.rs
- theos-ai/src/context.rs
- lore-poc fixture
- Chakra
- LoreRecord
- hash.rs
- cast.rs
- plane.rs
- TheosAppHandler
- .assemble
- Review Focus
- injection.rs
- bt-poc fixture
- Review Focus
- compare_rows.py
- NPC prompt injection: baseline and guardrail proposal
- Behaviour-tree PoC and its injection suite — design
- moderation/README.md
- reply.rs
- lore_poc.rs
- CountingModule<Tag>
- choose_output
- extract.rs
- log_format.rs
- kernel_integration.rs
- strip_ansi
- score.py
- retrieval.rs
- mini
- embed.rs
- RecordId
- npc-poc/src/inject.rs
- Metrics
- Clock
- theos-ai/src/lib.rs
- MemoryProbe

## God Nodes (most connected - your core abstractions)
1. `Vec3` - 92 edges
2. `rig()` - 59 edges
3. `RecordId` - 53 edges
4. `mini()` - 41 edges
5. `TheosKernel` - 38 edges
6. `Vec4` - 38 edges
7. `load_default()` - 37 edges
8. `request()` - 35 edges
9. `Vec2` - 34 edges
10. `run_model()` - 33 edges

## Surprising Connections (you probably didn't know these)
- `Type-Erased EventBus via ChannelHandle adapter` --semantically_similar_to--> `Service Locator encapsulated in KernelContext`  [INFERRED] [semantically similar]
  theos-core/src/event.rs → plans/THEOS-86.md
- `PhysicsEvent` --semantically_similar_to--> `GameEvent`  [INFERRED] [semantically similar]
  theos-physics/src/event.rs → theos-core/src/event.rs
- `select_rejects_an_unknown_speaker()` --calls--> `rig()`  [INFERRED]
  theos-ai/src/driver.rs → plugins/llm-poc/bt-poc/src/run.rs
- `answer_counts_thinking_and_answer_tokens_separately_and_parses_only_answer()` --calls--> `rig()`  [INFERRED]
  theos-ai/src/driver.rs → plugins/llm-poc/bt-poc/src/run.rs
- `answer_records_context_ids_from_retrieval()` --calls--> `rig()`  [INFERRED]
  theos-ai/src/driver.rs → plugins/llm-poc/bt-poc/src/run.rs

## Import Cycles
- 2-file cycle: `theos-ai/src/driver.rs -> theos-ai/src/reply.rs -> theos-ai/src/driver.rs`

## Hyperedges (group relationships)
- **Unsafe Code Audit Triad (Clippy, Forbid, Geiger)** — github_workflows_check_lint_clippy_unsafe_policy_job, github_workflows_check_lint_forbid_unsafe_verification, github_workflows_check_lint_cargo_geiger [EXTRACTED 0.95]
- **Cross-Module Service Discovery via KernelContext: ProviderModule registers, ConsumerModule retrieves** — theos_core_tests_kernel_integration_providermodule, theos_core_tests_kernel_integration_consumermodule, theos_core_tests_kernel_integration_greetingservice [EXTRACTED 1.00]
- **Type-Erased EventBus: CrossbeamEventBus stores ChannelHandle<TypedChannel<E>> keyed by TypeId** — src_lib_crossbeameventbus, src_lib_channelhandle, theos_core_src_event_typedchannel_e [EXTRACTED 1.00]
- **Kernel Lifecycle: Register → DependencyResolver sort → init → fixed-step loop → shutdown** — theos_core_src_kernel_theoskernel, theos_core_src_dependency_resolver_dependencyresolver, concept_fixed_step_loop [EXTRACTED 1.00]
- **Release Artifact Production (Launcher, Game, Editor)** — github_workflows_release_packaging_theos_launcher_artifact, github_workflows_release_packaging_theos_game_artifact, github_workflows_release_packaging_theos_editor_artifact [EXTRACTED 1.00]
- **CI Quality Gate Pipeline (Check, Lint, Test)** — github_workflows_check_lint_check_lint_workflow, github_workflows_test_test_workflow, github_workflows_check_lint_clippy_unsafe_policy_job [INFERRED 0.85]

## Communities (303 total, 40 thin omitted)

### Community 0 - "run_cli"
Cohesion: 0.11
Nodes (28): schema_compiles_in_llguidance_for_every_scenario(), defects(), fixture_matches_committed_snapshots(), model_path_display(), moderation_label(), Row, BTreeMap, Duration (+20 more)

### Community 1 - "CloseToExitModule"
Cohesion: 0.16
Nodes (17): CloseToExitModule, make_initialised_close_module(), Arc, Box, Default, Error, Option, Receiver (+9 more)

### Community 2 - "theos-core/src/event.rs"
Cohesion: 0.10
Nodes (8): BroadcastBusExt, Debug, E, HashMap, RwLock, Sender, T, TypedChannel

### Community 3 - "dependency_resolver.rs"
Cohesion: 0.07
Nodes (31): Kahn's Topological Sort for Module Init Order, ModuleResult, DependencyResolver, five_module_mixed_graph_satisfies_all_dependencies(), linear_chain_resolves_in_dependency_order(), module(), module_with_deps(), Box (+23 more)

### Community 4 - "TheosKernel"
Cohesion: 0.12
Nodes (16): Fixed-Step Game Loop (60 Hz sleep+spin hybrid), Service Locator encapsulated in KernelContext, Type-Erased EventBus via ChannelHandle adapter, KernelResult, ChannelHandle (private trait), CrossbeamEventBus, THEOS-86 Microkernel Bootstrap Plan, allocation_check_typed_event_bus (alloc test) (+8 more)

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
Cohesion: 0.12
Nodes (34): calling_init_twice_returns_already_initialized(), default_constructs_kernel_with_no_modules_and_uninitialised_flag(), event_bus_handle_returns_clone_of_kernel_bus(), init_calls_module_init_in_dependency_sorted_order(), init_propagates_dependency_resolver_error_without_running_any_module(), init_stops_at_first_failing_module_and_skips_later_modules(), init_with_no_modules_succeeds_and_marks_kernel_initialised(), registering_duplicate_name_errors_and_leaves_buffer_unchanged() (+26 more)

### Community 10 - ".new"
Cohesion: 0.16
Nodes (21): test_glam_vec2_roundtrip(), test_glam_vec3_and_vec3a_produce_same_components(), test_glam_vec3_roundtrip(), test_glam_vec3a_roundtrip(), test_glam_vec4_roundtrip(), test_vec2_constructors_and_dot(), test_vec2_dot_antiparallel_is_negative(), test_vec2_dot_orthogonal_is_zero() (+13 more)

### Community 11 - "prompt.rs"
Cohesion: 0.06
Nodes (95): BtScenario, enemy_front_matter_reader_splits_id_title_and_body(), EnemyRecord, Entity, Fixture, fixture_loads_20_scenarios_and_4_enemies(), load_default(), parse_enemy() (+87 more)

### Community 12 - "Release Packaging Job"
Cohesion: 0.09
Nodes (23): Cargo Plugins Workspace Dependency Updates (theos-plugin-deps), Cargo Workspace Dependency Updates (theos-engine-deps), Dependabot Configuration, GitHub Actions Dependency Updates, cargo-geiger Unsafe Audit Tool, Cargo Check Job, Check and Lint Workflow, Clippy and Unsafe Policy Job (+15 more)

### Community 13 - "upkeep.rs"
Cohesion: 0.06
Nodes (72): approx(), assemble(), body(), efficiency_matches_manual_eta_on_constructive_graph(), efficiency_on_zero_energy_graph_is_well_defined(), EnvResistance, full_transfer_graph_efficiency_near_one(), graph_energy_matches_manual_sum_over_edges() (+64 more)

### Community 14 - "EntityId"
Cohesion: 0.06
Nodes (45): AtomicU64, EntityMap, AiBrain, PendingAiResponse, String, component_ids_lists_inserted_component_types(), contains_is_true_for_spawned_entity(), despawn_removes_entity_and_its_components() (+37 more)

### Community 15 - "THEOS-86: Microkernel Core Bootstrap — Entry Point, Thread Pool & Kernel Initialization"
Cohesion: 0.04
Nodes (44): 1.1 Define `EngineModule` Trait (in `theos-shared`), 1.2 Define `EventBus` Trait (in `theos-shared`), 1.3 Create `ServiceLocator` Struct (in `theos-shared`), 1.4 Stub Out `TheosKernel` Struct (in `theos-core`), 1.5 Define `AiWorkerHandle` Struct (in `theos-core`), 1.6 Define `AiCommandQueue` Struct (in `theos-core`), 1.7 Design `ModuleRegistry` (Logical Component), 2.1 Logging Initialization (+36 more)

### Community 16 - "ServiceLocator"
Cohesion: 0.17
Nodes (14): ServiceLocatorResult, register_and_get_service(), Any, Arc, HashMap, Option, RwLock, Send (+6 more)

### Community 17 - "bt-poc/src/run.rs"
Cohesion: 0.08
Nodes (64): FnMut, Cli, Command, default_moderation(), main(), Command, Option, PathBuf (+56 more)

### Community 18 - "Self"
Cohesion: 0.11
Nodes (3): Output, Self, test_simd_interop()

### Community 19 - "worker.rs"
Cohesion: 0.11
Nodes (32): AtomicUsize, Completed, F, JoinHandle, counting_worker(), CountingBackend, drop_discards_queued_requests(), drop_with_request_in_flight_joins() (+24 more)

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
Cohesion: 0.20
Nodes (14): Cli, Command, default_fixture(), default_moderation(), export(), main(), Command, Option (+6 more)

### Community 27 - ".record_debug"
Cohesion: 0.40
Nodes (4): Field, FieldVisitor<'a, 'w>, Debug, Visit

### Community 28 - "Strict DAG No-Circular-Dependency Crate Layout"
Cohesion: 0.50
Nodes (4): Strict DAG No-Circular-Dependency Crate Layout, theos-shared Zero-Internal-Dependency Policy, Workspace Design Principles (README), theos-shared EntityId Design

### Community 29 - "benches/event_bus.rs"
Cohesion: 0.83
Nodes (3): bench_send_then_recv_boxed_baseline(), bench_send_then_recv_typed(), Criterion

### Community 30 - "BroadcastEventBus"
Cohesion: 0.26
Nodes (11): BroadcastEventBus, Any, Box, EventBusResult, Formatter, Option, Result, Send (+3 more)

### Community 31 - "EventBus"
Cohesion: 0.16
Nodes (16): Test2Event, TestEvent, PhysicsEvent, ChannelHandle, Event, EventBus, EventBusExt, Clone (+8 more)

### Community 32 - "ModuleDependency"
Cohesion: 0.18
Nodes (7): FailingInitModule, RecordingModule<Tag>, TypeId, ModuleDependency, Self, TypeId, VersionReq

### Community 33 - "knn.rs"
Cohesion: 0.05
Nodes (101): StubModerator, CheckRecord, Duration, allowed_records_without_a_nearest_banned_are_listed_at_the_end(), closest_allowed_is_ordered_by_distance_not_by_banned_similarity(), header_reports_check_count_median_and_p95_per_source(), md_cell(), near() (+93 more)

### Community 34 - "THEOS-181 — [magic] E16c construction transient + per-chakra throughput meter / overdraw"
Cohesion: 0.15
Nodes (12): Acceptance criteria (from Jira), Canon (Equations of the Weave, E16c — pinned + the gap this story fills), Canon writeback (E16c has a genuine gap — for the canon-authority agent), Coverage note, Current code state (all inputs already exist), Decision register — LOCKED (2026-08-29, guided session), Goal (one sentence), Implementation notes (2026-08-29, guided) (+4 more)

### Community 35 - ".beliefs"
Cohesion: 0.43
Nodes (4): corpus_beliefs_yields_only_beliefs_in_id_order(), corpus_records_yields_every_record_in_id_order(), Item, Iterator

### Community 36 - "CallLog"
Cohesion: 0.36
Nodes (5): CallLog, RecordingModule, Mutex, PhantomData, Vec

### Community 37 - "transient.rs"
Cohesion: 0.17
Nodes (41): approx(), assemble(), body(), construct_ids_are_unique_across_spawns(), ConstructId, overdrawn_clears_after_the_transient_decays(), overdrawn_flags_an_anchor_when_floor_plus_transient_exceeds_channel_power(), overdrawn_is_empty_when_every_anchor_is_within_channel_power() (+33 more)

### Community 38 - "Tag"
Cohesion: 0.19
Nodes (12): moderator(), recording_moderator_records_every_check_with_its_tag(), RecordingModerator, Records, Arc, Into, Mutex, Result (+4 more)

### Community 39 - "WindowError"
Cohesion: 0.50
Nodes (3): OsError, EventLoopError, WindowError

### Community 51 - "theos-shared"
Cohesion: 0.13
Nodes (13): code:powershell (cd "D:\Projects\Tower of Theos\theos-engine"), code:rust (// explicit import), code:mermaid (flowchart TB), Contribution notes, Design principles, EntityId (canonical decision), Open decisions / TODO, Planned public surface (+5 more)

### Community 52 - "THEOS-12: Initialization Phase — extract `Kernel::init()`"
Cohesion: 0.17
Nodes (11): AC traceability, Changes, Confirmed decisions (vs. spec wording), Goal, Other deltas vs. spec wording, Out of scope (deferred), Risks, Tests (`theos-core/src/kernel.rs` `mod tests`) (+3 more)

### Community 53 - "EscapeToQuitModule"
Cohesion: 0.15
Nodes (18): EscapeToQuitModule, make_initialised_escape_module(), Arc, Box, Default, Error, Option, Receiver (+10 more)

### Community 56 - "LlamaDecoder"
Cohesion: 0.24
Nodes (8): ends_thinking(), LlamaDecoder, LlamaBatch, LlamaSampler, Matcher, Option, sampler_chain(), Utf8Stream

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
Cohesion: 0.16
Nodes (16): Deref, Target, Any, Box, EventBusResult, Send, Sync, TypeId (+8 more)

### Community 70 - "THEOS-147 — [seam] ECS World access trait + in-memory stub + AI components"
Cohesion: 0.22
Nodes (8): Acceptance-criteria gap table, Crate wiring (done), Deferred / out of scope, Locked decisions, Related, Resolved design decisions (were open during design), Test execution / CI, THEOS-147 — [seam] ECS World access trait + in-memory stub + AI components

### Community 72 - "theos-window"
Cohesion: 0.33
Nodes (5): Build, Platform requirements, Public API, Testing, theos-window

### Community 74 - "theos-ai-llama/tests/real_model.rs"
Cohesion: 0.17
Nodes (30): embedder(), embedder_ranks_related_lore_above_unrelated(), embedder_returns_unit_vectors_of_dimension_1024(), forged_turn_markers_become_control_tokens(), generator(), generator_closes_thinking_at_the_budget(), generator_emits_thinking_pieces_then_answer_when_thinking_on(), generator_reply_parses_with_theos_ai_parse_reply() (+22 more)

### Community 78 - "bt_spike.rs"
Cohesion: 0.17
Nodes (25): children(), condition(), fixture(), guided(), int(), main(), matcher_for(), measure() (+17 more)

### Community 79 - "ElementField"
Cohesion: 0.13
Nodes (9): element_field_round_trips_through_json(), ElementField, ElementKind, Index, IndexMut, Item, Iterator, Self (+1 more)

### Community 80 - "npc-poc/src/run.rs"
Cohesion: 0.18
Nodes (36): a_failing_request_becomes_a_failed_row_and_the_run_continues(), a_failing_warm_up_is_reported_and_the_run_goes_on(), a_worker_that_dies_mid_run_returns_the_rows_so_far_and_a_fatal_error(), belief(), defects_name_rows_with_a_leak_or_forbidden_record(), every_finished_row_is_on_disk_in_rows_jsonl(), finished_rows_write_moderation_ms_and_no_elapsed_ms(), flaky_worker() (+28 more)

### Community 81 - "THEOS-148 — [M1][magic] theos-magic crate scaffold & core HarmonicToken seam"
Cohesion: 0.29
Nodes (6): Acceptance-criteria gap table, Deferred, Locked decisions, Resolved decisions (chakra & element model), Test execution, THEOS-148 — [M1][magic] theos-magic crate scaffold & core HarmonicToken seam

### Community 83 - "theos-ai"
Cohesion: 0.80
Nodes (5): bt-poc, guard-kit, npc-poc, theos-ai, theos-ai-llama

### Community 84 - "assembler.rs"
Cohesion: 0.16
Nodes (34): approx(), assemble(), body(), cast_time_equals_critical_path_delay_when_no_transfers_present(), cast_time_on_acyclic_graph_matches_manual_calc(), cast_time_on_cyclic_graph_matches_manual_calc(), chooses_lowest_index_chakra_as_injection_root(), convergent_node_amplitude_adds_when_in_phase() (+26 more)

### Community 86 - "Locked decisions (design session 2026-06-13)"
Cohesion: 0.17
Nodes (11): AC → file gap table, Chakra layer (`chakra.rs`), Coverage target, Deferred / out of scope, Graph layer (`graph/`), Locked decisions (design session 2026-06-13), Scaffolding contract, Seam ripple (`interface.rs`, `error.rs`) (+3 more)

### Community 87 - "Vec4"
Cohesion: 0.09
Nodes (19): Add, AddAssign, Div, DivAssign, f32x4, Sub, SubAssign, glam::Vec2 (+11 more)

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

### Community 101 - "driver.rs"
Cohesion: 0.21
Nodes (21): answer_counts_thinking_and_answer_tokens_separately_and_parses_only_answer(), answer_keeps_highest_vram_sample(), answer_pieces(), generate_honours_its_token_cap(), generate_passes_none_schema_to_the_backend(), generate_passes_the_thinking_budget_to_the_backend(), generate_runs_a_prompt_without_retrieval_or_parsing(), GenerateJob (+13 more)

### Community 102 - "theos-ai/src/testkit.rs"
Cohesion: 0.10
Nodes (29): FullOnce, Flaky, Backend, fake_embedder_is_deterministic(), fake_embedder_scores_shared_words_higher(), FakeClock, FakeEmbedder, FakeProbe (+21 more)

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

### Community 120 - "npc-poc/src/report.rs"
Cohesion: 0.08
Nodes (87): a_deadline_cut_row_is_not_a_words_pass(), a_driver_failed_row_does_not_lower_leak_or_forbidden(), a_resist_row_without_a_reply_is_an_error_not_held(), answers_list_forged_markers_apart_from_the_reply_signals(), answers_mark_hand_read_scenarios_and_missing_replies(), answers_show_good_outcome_and_read_by_hand_for_s1(), behaviour_label(), cell() (+79 more)

### Community 121 - "spawn_worker"
Cohesion: 0.12
Nodes (12): Guards, guards_parse_presets_and_lists_and_reject_unknown_ids(), Result, String, context_options(), guards_map_to_context_options(), LoadInfo, Arc (+4 more)

### Community 122 - "gain.rs"
Cohesion: 0.37
Nodes (11): approx(), gain_differs_with_environment_on_same_graph(), gain_matches_manual_q_for_known_s_and_gamma(), gain_near_one_for_well_damped_graph(), gain_rises_as_stability_margin_thins(), ResonanceRunaway, Result, runaway_when_stability_nonpositive() (+3 more)

### Community 123 - "npc-poc/src/score.rs"
Cohesion: 0.07
Nodes (67): a_resist_scenario_without_severity_or_technique_fails_to_load(), fixture(), load_scenarios(), load_visible(), Markers, parse_scenarios(), BTreeMap, Display (+59 more)

### Community 124 - "bt-poc/src/report.rs"
Cohesion: 0.09
Nodes (63): Verdict, by_severity(), by_technique(), checks_column_shows_how_many_gate_passing_requests_it_is_over(), csv(), csv_and_request_table_append_moderation_ms_and_elapsed_ms(), csv_field(), csv_quotes_fields_with_commas_and_has_one_line_per_row() (+55 more)

### Community 125 - "tune.rs"
Cohesion: 0.07
Nodes (65): CategoryList, fixture(), ListFile, load_lists(), load_lists_reads_a_temp_fixture(), load_lists_rejects_overlap_and_duplicates_and_empty(), ModerationLists, read_list() (+57 more)

### Community 126 - "ModelContext"
Cohesion: 0.13
Nodes (18): EmbeddingsSeqError, LlamaContext, LlamaContextParams, LlamaError, LlamaRuntime, ModelContext, Arc, DecodeError (+10 more)

### Community 127 - "Npc"
Cohesion: 0.17
Nodes (12): answer_records_context_ids_from_retrieval(), FullBackend, FullDecoder, generate_stops_at_the_deadline(), Npc, NpcError, NpcParts, Box (+4 more)

### Community 128 - "WovenToken"
Cohesion: 0.17
Nodes (10): kinematic(), kinematic_vector_into_an_effector_is_allowed(), body(), Vec, spatial(), HarmonicToken, Vec, TokenId (+2 more)

### Community 129 - "NPC LLM proof of concept — design"
Cohesion: 0.08
Nodes (23): 10. Order of work, 11. Left to the builder, 12. Calls made by the designer (not discussed with the owner), 1. Intent, 2. Goals, non-goals, anti-goals, 3. Fixed decisions (settled before this session — do not reopen), 4. Decided in this session, 5. Findings from recon (2026-11-13) (+15 more)

### Community 130 - "TheosFormatter"
Cohesion: 0.14
Nodes (14): FmtContext, FormatEvent, N, Rfc3339, S, logging::init(), FieldVisitor, init() (+6 more)

### Community 131 - "Review Focus"
Cohesion: 0.11
Nodes (18): Global Constraints, NPC Guardrails Implementation Plan, Results, Review Focus, Task 10: `npc-poc` guards, moderation outcomes and tuning, Task 11: `bt-poc` guards and moderation outcomes, Task 12: Tune, then stop for the owner's review (controller), Task 13: Measurement (controller) (+10 more)

### Community 132 - "rig"
Cohesion: 0.16
Nodes (37): rig(), allow_all_answers_exactly_as_before(), answer_errors_on_unknown_speaker(), answer_measures_time_to_first_token_total_and_tokens_per_second(), answer_passes_no_thinking_budget(), answer_passes_player_name_and_extra_instruction_to_the_assembler(), answer_passes_prompt_schema_and_thinking_to_backend(), answer_returns_parsed_reply_and_cited_ids() (+29 more)

### Community 133 - "File structure"
Cohesion: 0.11
Nodes (18): Deferred, File structure, Global Constraints, NPC LLM Proof of Concept Implementation Plan, Review Focus, Spike results, Task 0: Branch setup (controller), Task 10: Harness `run` — scoring and results tables (+10 more)

### Community 134 - "NPC guardrails: content moderation and the input channel — design"
Cohesion: 0.12
Nodes (16): 1. Intent, 2. Goals, non-goals, anti-goals, 3. Constraints (carried over, binding), 4. What the owner decided, 5.1 Moderation API (`theos-ai`, new module `moderation`), 5.2 Outcomes in the NPC driver, 5.3 Trust labels (G8), name channel (G9), spotlighting (G4), 5.4 Safe tokenization (G1, `theos-ai-llama`) (+8 more)

### Community 135 - "generator.rs"
Cohesion: 0.13
Nodes (23): decode_error(), ModelEntry, ModelProfile, models_toml_parses_profile(), models_toml_parses_tree_sampling_and_n_ctx(), models_toml_rejects_missing_sampling(), models_toml_safe_untrusted_defaults_off(), ModelsFile (+15 more)

### Community 136 - "bt-poc/src/score.rs"
Cohesion: 0.13
Nodes (33): OsString, a_hanging_python_is_killed_at_the_deadline(), an_unknown_scenario_id_is_a_real_error(), check_py_crash_becomes_a_crash_gate_row(), clip(), fixture_dir(), leak(), name_overlap() (+25 more)

### Community 137 - "check.py"
Cohesion: 0.11
Nodes (25): Exception, check_params(), check_shape(), check_value(), evaluate(), event_key(), event_matches(), gate() (+17 more)

### Community 138 - "ShutdownRequester"
Cohesion: 0.23
Nodes (12): clone_shares_state_with_original(), default_equals_new(), handle_flip_is_visible_to_typed_api(), handle_shares_state_with_typed_api(), new_starts_unrequested(), request_shutdown_is_idempotent(), request_shutdown_sets_flag(), Arc (+4 more)

### Community 139 - "Chakras"
Cohesion: 0.11
Nodes (17): Attunement, chakra_state_round_trips_through_json(), Chakras, chakras_default_is_unattuned_and_balanced(), ChakraState, from_array_preserves_canonical_order(), indexing_and_iteration_observe_the_same_state(), iter_mut_allows_mutation_of_every_point() (+9 more)

### Community 140 - "Vec3"
Cohesion: 0.22
Nodes (9): f32, glam::Vec3, glam::Vec3A, mint::Vector3<f32>, Mul, Vec3, Vec3Swizzle, Vec3A (+1 more)

### Community 141 - "ray.rs"
Cohesion: 0.33
Nodes (11): assert_vec3_eq(), Ray, Self, test_at_known_t_value(), test_at_negative_t_goes_behind_origin(), test_at_one_returns_origin_plus_direction(), test_at_zero_returns_origin(), test_new_accepts_axis_aligned_directions() (+3 more)

### Community 142 - "embedder.rs"
Cohesion: 0.16
Nodes (18): Cow, document_text_is_embedded_as_given(), ensure_ends_with(), ensure_ends_with_appends_missing_end_token_once(), ensure_ends_with_handles_empty_sequence(), input_text(), l2_normalize(), l2_normalize_leaves_zero_vector_zero() (+10 more)

### Community 143 - "LlamaGenerator"
Cohesion: 0.12
Nodes (21): LlamaChatTemplate, FullDecoder, Box, Box, LlamaGenerator, Arc, Box, LlamaToken (+13 more)

### Community 144 - "bt-poc/src/inject.rs"
Cohesion: 0.11
Nodes (60): a_baseline_from_a_different_setup_is_warned_about_in_the_injection_header(), a_baseline_gate_failure_with_the_same_rule_is_inconclusive_not_a_defence(), a_baseline_with_another_seed_or_fixture_hash_is_warned_about(), a_scenario_with_no_catching_check_cannot_hold_when_nothing_can_trip(), an_obeying_tree_trips_bj02s_melee_and_charge_checks(), an_obeying_tree_trips_bj05s_never_target_warden_2_check(), an_obeying_tree_trips_bj07s_charge_and_melee_checks(), assert_obeying_tree_is_caught() (+52 more)

### Community 145 - "spike.rs"
Cohesion: 0.31
Nodes (10): dxgi_usage(), generate(), main(), print_dxgi(), LlamaBackend, LlamaModel, LlamaSampler, Option (+2 more)

### Community 146 - "theos-ai/src/context.rs"
Cohesion: 0.11
Nodes (46): all_off_without_name_or_instruction_is_unchanged(), assemble(), assemble_drops_lore_ids_the_speaker_may_not_see(), assemble_includes_persona_bodies_by_default(), assemble_lists_held_under_what_you_know_with_bracketed_ids(), assemble_lists_rejected_under_what_you_reject(), assemble_marks_only_the_question_untrusted(), assemble_names_speaker_role_and_lens() (+38 more)

### Community 147 - "lore-poc fixture"
Cohesion: 0.20
Nodes (9): Checks run, Layout, lore-poc fixture, Notes on the sources, Scenario fields, Scoring, The speaker rule, What the fixture adds (+1 more)

### Community 148 - "Chakra"
Cohesion: 0.31
Nodes (5): Chakra, Construct, Lane, Vec, ThroughputMeter

### Community 149 - "LoreRecord"
Cohesion: 0.17
Nodes (28): b942_voice_has_saying_without_header(), binder_speaker_has_two_layers_and_throat_kin_lens(), body_text(), convert_lore(), convert_speaker(), count(), fixture_converts_to_beliefs_truths_clues_topics_and_eight_speakers(), holder_kind() (+20 more)

### Community 268 - "hash.rs"
Cohesion: 0.28
Nodes (5): file_sha256(), Path, Result, String, sha256_hex()

### Community 269 - "cast.rs"
Cohesion: 0.22
Nodes (22): approx(), body(), CannotChannel, caster(), caster_with_root_power(), effective_cast_time_is_energy_time_when_throughput_bound(), effective_cast_time_is_settling_when_propagation_bound(), frozen_cast_time_unchanged_by_floor() (+14 more)

### Community 270 - "plane.rs"
Cohesion: 0.26
Nodes (15): assert_vec3_eq(), Plane, Option, Self, test_intersects_ray_from_above_hits_origin(), test_intersects_ray_from_below_hits_plane(), test_intersects_ray_offset_origin(), test_intersects_ray_parallel_returns_none() (+7 more)

### Community 271 - "TheosAppHandler"
Cohesion: 0.27
Nodes (7): Arc, AtomicBool, EventLoop, EventLoopError, Result, Self, TheosAppHandler

### Community 272 - ".assemble"
Cohesion: 0.10
Nodes (13): CompilerContext, CompilerError, String, assemble_default_uses_the_default_wave_speed(), GraphAssembler, Result, TensionGraph, edge_delay() (+5 more)

### Community 273 - "Review Focus"
Cohesion: 0.12
Nodes (15): Behaviour-Tree PoC Implementation Plan, Deferred, Global Constraints, Results, Review Focus, Spike results, Task 1: Spike — a recursive tree schema through llguidance (controller-reviewed, no tests), Task 2: `theos-ai` — optional grammar, `Npc::select`, `Npc::generate` (+7 more)

### Community 274 - "injection.rs"
Cohesion: 0.32
Nodes (5): injection_attenuation(), injection_attenuation_composes_multiplicatively(), injection_attenuation_strictly_decreases_with_depth(), kinematic_source_amplitude(), Option

### Community 275 - "bt-poc fixture"
Cohesion: 0.14
Nodes (13): A request, bt-poc fixture, check.py, Checks, Checks run, How a tick runs, Layout, Run conditions (+5 more)

### Community 276 - "Review Focus"
Cohesion: 0.15
Nodes (12): Baseline results, bt-poc injection sketches (format of `fixtures/bt-poc/scenarios.toml`, not built), Deferred, Global Constraints, Guardrail proposal (for decision, nothing built), NPC Prompt-Injection Suite Implementation Plan, Review Focus, Task 0: Branch, fixture and plan (controller) (+4 more)

### Community 277 - "compare_rows.py"
Cohesion: 0.60
Nodes (4): load(), main(), Path, Compare two harness results folders row by row (determinism check). Usage:…

### Community 278 - "NPC prompt injection: baseline and guardrail proposal"
Cohesion: 0.17
Nodes (11): 10. Deferred, 1. Where things are, 2. What the system is, 3. Research summary: how injections are written and what they damage, 4. The suite, 5. Baseline (no guardrails, special-token hole open), 6. Fix the suite before measuring guardrails, 7. Guardrail proposal (+3 more)

### Community 279 - "Behaviour-tree PoC and its injection suite — design"
Cohesion: 0.17
Nodes (11): 10. Calls made by the designer (not discussed with the owner), 1. Intent, 2. Goals, non-goals, anti-goals, 3. Decided, 4. `theos-ai` change (minimal, general), 5. The harness `plugins/llm-poc/bt-poc` (disposable), 6. Run conditions, 7. BT injection suite (+3 more)

### Community 281 - "reply.rs"
Cohesion: 0.20
Nodes (20): ids(), parse_reply(), parse_reply_decodes_escapes_and_ignores_surrounding_whitespace(), parse_reply_drops_duplicate_sources_keeping_order(), parse_reply_reads_sources_and_text(), parse_reply_rejects_blank_reply(), parse_reply_rejects_extra_keys_and_bad_json(), parse_reply_rejects_source_outside_context() (+12 more)

### Community 282 - "lore_poc.rs"
Cohesion: 0.17
Nodes (15): Vec, Adversarial, fixture(), Golden, no_forbidden_record_reaches_any_prompt(), Result, Scenario, String (+7 more)

### Community 284 - "choose_output"
Cohesion: 0.46
Nodes (7): choose_output(), defaults_to_info_when_rust_log_missing(), invalid_rust_log_does_not_crash_and_falls_back(), Output, String, Vec, strip_ansi()

### Community 285 - "extract.rs"
Cohesion: 0.43
Nodes (4): balanced_end(), first_json_object(), Option, Value

### Community 286 - "log_format.rs"
Cohesion: 0.57
Nodes (6): choose_output(), module_path_is_present(), Output, String, startup_line_matches_layout(), strip_ansi()

### Community 287 - "kernel_integration.rs"
Cohesion: 0.10
Nodes (28): ConsumerModule, CountingModule, GreetingService, kernel_runs_init_tick_shutdown_lifecycle(), make_counting_module(), module_retrieves_service_registered_by_its_dependency(), ProviderModule, reports_frame_timing_accuracy() (+20 more)

### Community 288 - "strip_ansi"
Cohesion: 0.67
Nodes (3): full_lifecycle_emits_expected_log_sequence(), String, strip_ansi()

### Community 294 - "retrieval.rs"
Cohesion: 0.13
Nodes (30): EmbedError, EmbedInput, String, build_embeds_only_beliefs(), build_errors_when_embedder_returns_vectors_of_differing_dimensions(), build_errors_when_embedder_returns_wrong_vector_count(), built(), check_count() (+22 more)

### Community 296 - "mini"
Cohesion: 0.35
Nodes (12): VisibleEntry, visible(), visible_excludes_a_rejected_belief_listed_in_unaware_of(), visible_excludes_unaware_of_ids(), visible_held_excludes_beliefs_of_other_holders(), visible_held_includes_beliefs_held_by_any_layer(), visible_ignores_rejects_ids_missing_from_corpus(), visible_lists_a_belief_both_held_and_rejected_only_as_rejected() (+4 more)

### Community 297 - "embed.rs"
Cohesion: 0.16
Nodes (15): Again, Calls, RefCell, Reentrant, Box, Cell, Option, Rc (+7 more)

### Community 298 - "RecordId"
Cohesion: 0.08
Nodes (33): layer_of(), push_layer_line(), push_lens_line(), push_persona(), Option, layer_of(), ids(), Item (+25 more)

### Community 299 - "npc-poc/src/inject.rs"
Cohesion: 0.21
Nodes (12): InjectingBackend, Box, Result, Self, String, Vec, script(), scripted() (+4 more)

### Community 303 - "Metrics"
Cohesion: 0.38
Nodes (9): GenerateResponse, LoopOutcome, Metrics, NpcRequest, NpcResponse, Duration, Option, String (+1 more)

### Community 304 - "Clock"
Cohesion: 0.53
Nodes (4): Clock, Instant, system_clock_never_goes_backwards(), SystemClock

### Community 307 - "MemoryProbe"
Cohesion: 0.38
Nodes (4): IDXGIAdapter3, DxgiProbe, Option, MemoryProbe

## Knowledge Gaps
- **452 isolated node(s):** `theos-audio`, `theos-assets`, `TagA`, `TagB`, `TagC` (+447 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **40 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `EntityId` connect `EntityId` to `.assemble`, `KernelError`?**
  _High betweenness centrality (0.208) - this node is a cross-community bridge._
- **Why does `Tag` connect `Tag` to `run_cli`, `dependency_resolver.rs`, `CallLog`, `npc-poc/src/run.rs`, `bt-poc/src/run.rs`, `kernel_integration.rs`?**
  _High betweenness centrality (0.119) - this node is a cross-community bridge._
- **Why does `Vec3` connect `Vec3` to `WovenToken`, `stability.rs`, `transient.rs`, `mat.rs`, `quat.rs`, `aabb.rs`, `.new`, `Chakras`, `upkeep.rs`, `cast.rs`, `plane.rs`, `ray.rs`, `Self`, `Chakra`, `assembler.rs`, `Vec4`, `NodeId`, `gain.rs`?**
  _High betweenness centrality (0.107) - this node is a cross-community bridge._
- **Are the 39 inferred relationships involving `rig()` (e.g. with `allow_all_answers_exactly_as_before()` and `answer_counts_thinking_and_answer_tokens_separately_and_parses_only_answer()`) actually correct?**
  _`rig()` has 39 INFERRED edges - model-reasoned connections that need verification._
- **Are the 38 inferred relationships involving `mini()` (e.g. with `all_off_without_name_or_instruction_is_unchanged()` and `assemble_text_is_unchanged_by_segments()`) actually correct?**
  _`mini()` has 38 INFERRED edges - model-reasoned connections that need verification._
- **What connects `theos-audio`, `theos-assets`, `TagA` to the rest of the system?**
  _452 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `run_cli` be split into smaller, more focused modules?**
  _Cohesion score 0.11494252873563218 - nodes in this community are weakly interconnected._