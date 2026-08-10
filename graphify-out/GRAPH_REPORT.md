# Graph Report - .  (2026-07-13)

## Corpus Check
- cluster-only mode — file stats not available

## Summary
- 485 nodes · 917 edges · 64 communities (26 shown, 38 thin omitted)
- Extraction: 85% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 39 edges (avg confidence: 0.52)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `ba48a2c0`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- Verb Operations
- Music Theory & Architecture
- Core Language Engine
- Audio & Test Foundation
- Integration Tests
- Parity & Compatibility
- Scale & Arpeggio
- Audio Effects
- Runtime Testing
- WASM Bindings
- Community 10
- Community 11
- Community 12
- Community 13
- Community 14
- Community 15
- Community 16
- Community 17
- Community 18
- Community 19
- Community 20
- Community 21
- Community 22
- Community 23
- Community 24
- Community 25
- Community 26
- Community 27
- Community 28
- Community 29
- Community 30
- Community 31
- Community 32
- Community 33
- Community 34
- Community 35
- Community 36
- Community 37
- Community 38
- Community 39
- Community 40
- Community 41
- Community 42
- Community 43
- Community 44
- Community 45
- Community 46
- Community 47
- Community 48
- Community 49
- Community 50
- Community 51
- Community 52
- Community 53
- Community 54
- Community 55
- Community 56
- Community 57
- Community 58
- Community 61
- Community 62

## God Nodes (most connected - your core abstractions)
1. `parse_key()` - 28 edges
2. `expand_euclid()` - 24 edges
3. `schedule_bar_seeded()` - 23 edges
4. `emit_token()` - 19 edges
5. `Voice` - 18 edges
6. `schedule_bar_keyed()` - 16 edges
7. `Key` - 16 edges
8. `sched_prog()` - 15 edges
9. `effective_of()` - 13 edges
10. `Program` - 13 edges

## Surprising Connections (you probably didn't know these)
- `main()` --calls--> `schedule_bar_keyed()`  [INFERRED]
  loom-tui/src/main.rs → loom-core/src/lib.rs
- `main()` --calls--> `parse_key()`  [INFERRED]
  loom-tui/src/main.rs → loom-core/src/lib.rs
- `wave_name()` --references--> `Wave`  [EXTRACTED]
  loom-wasm/src/lib.rs → loom-core/src/lib.rs
- `LOOM Language` ----> `AI Brain`  [HIGH]
  README.md → CLAUDE.md
- `LOOM Language` ----> `DAW Mode (Studio)`  [MEDIUM]
  README.md → ROADMAP.md

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **TUTORIEL::drum-rhythm-flow** — TUTORIEL::drums, TUTORIEL::verbs, NIGHT_LOG::euclidean-rhythms, NIGHT_LOG::ratchets-rolls, NIGHT_LOG::probability, NIGHT_LOG::seed-determinism [INFERRED]
- **TUTORIEL::melody-harmonic-flow** — TUTORIEL::melody, CLAUDE::key-tonality, TUTORIEL::scale-degrees, NIGHT_LOG::gammes-du-monde, NIGHT_LOG::accords, NIGHT_LOG::arpeges [INFERRED]
- **TUTORIEL::synth-design-pipeline** — TUTORIEL::waveforms, TUTORIEL::fm-synthesis, TUTORIEL::supersaw, TUTORIEL::filters, TUTORIEL::envelope-adsr, TUTORIEL::lfo, TUTORIEL::distortion-drive, TUTORIEL::reverb-delay [INFERRED]
- **ROADMAP::long-form-composition** — NIGHT_LOG::song-structure, NIGHT_LOG::section-tagging, TUTORIEL::tempo-meter, TUTORIEL::variation-ab, ROADMAP::motif-development [INFERRED]
- **NIGHT_LOG::production-workflow** — CLAUDE::pattern-syntax, TUTORIEL::verbs, TUTORIEL::filters, NIGHT_LOG::sidechain-duck, NIGHT_LOG::wav-export, NIGHT_LOG::midi-output [INFERRED]
- **Synthesis & Sound Design Layer** — synthesis_system, sound_design_fx, filter_system, lfo_system, modulation_system, sample_playback, bus_master, sidechain_duck [INFERRED]
- **Music Theory Layer** — music_theory_layer, scale_degrees, world_scales, chord_system, roman_chord, arpeggio_system, microtonal_support [INFERRED]
- **Rhythm & Generative Layer** — generative_variation, euclidean_rhythm, probability_system, ratchet_roll, polyrhythm, song_structure, game_theory_xenakis [INFERRED]
- **Frontend Surfaces** — web_engine, loom_tui, interface_themes, tutorial_system, preset_system, visualization_system [INFERRED]
- **Night Build Complete Feature Set** — music_theory_layer, generative_variation, song_structure, sound_design_fx, recording_export, midi_support, preset_system, interface_themes, tutorial_system, visualization_system, bus_master [INFERRED]
- **Core Architecture** — loom_core, loom_wasm, web_audio_api, webgl_shaders, rust_testing, bit_parity [INFERRED]

## Communities (64 total, 38 thin omitted)

### Community 0 - "Verb Operations"
Cohesion: 0.10
Nodes (63): apply_verb(), arp_mode(), arp_order(), ArpMode, ca_row(), develop_evolves_each_bar(), effective(), effective_of() (+55 more)

### Community 1 - "Music Theory & Architecture"
Cohesion: 0.06
Nodes (41): AI Brain (VIBE), Arpeggio System, Bus Master (Mixing), Chord System, CLAUDE.md, Euclidean Rhythm, Filter System, Generative Variation (+33 more)

### Community 2 - "Core Language Engine"
Cohesion: 0.06
Nodes (13): alternation_not_misread_as_roll(), close(), decomment(), digits_after(), generators_match_js_engine(), js_fnv(), maths_generators(), parse_meter() (+5 more)

### Community 3 - "Audio & Test Foundation"
Cohesion: 0.07
Nodes (29): baseC, cm, cmaj, cmaj2, cmajT, ctxObj, document, els (+21 more)

### Community 4 - "Integration Tests"
Cohesion: 0.10
Nodes (27): accel_ramp_matches_js(), accents_set_velocity(), audio_only_directives_are_not_voices(), cellular_automaton_evolves_per_bar(), crescendo_roll_ramps_velocity(), euclidean_expands_to_tresillo(), euclidean_rotation_and_mixing(), event_carries_exact_microtonal_freq() (+19 more)

### Community 5 - "Parity & Compatibility"
Cohesion: 0.09
Nodes (22): anode(), ctxObj, document, els, fs, getEl(), html, localStorage (+14 more)

### Community 6 - "Scale & Arpeggio"
Cohesion: 0.09
Nodes (27): ab_variation_alternates_per_bar(), absolute_notes_still_work_with_a_key(), arp_down_and_updown_orders(), arp_spreads_chord_across_the_step(), chromatic_alteration_raises_and_lowers_a_degree(), degrade_and_chance_drop_seeded(), degrees_resolve_against_c_major(), extended_chords_and_inversions() (+19 more)

### Community 7 - "Audio Effects"
Cohesion: 0.08
Nodes (6): LoomFlange, LoomLim, LoomRedux, LoomReson, LoomSat, LoomWidth

### Community 8 - "Runtime Testing"
Cohesion: 0.12
Nodes (16): ctxObj, document, els, fs, getEl(), html, localStorage, makeEl() (+8 more)

### Community 9 - "WASM Bindings"
Cohesion: 0.24
Nodes (15): bar_bpm(), bar_seconds(), cachedTextDecoder, cachedTextEncoder, decodeText(), getStringFromWasm0(), getUint8ArrayMemory0(), initSync() (+7 more)

### Community 10 - "Community 10"
Cohesion: 0.22
Nodes (13): bar_bpm(), bar_seconds(), cachedTextDecoder, cachedTextEncoder, decodeText(), getStringFromWasm0(), getUint8ArrayMemory0(), passStringToWasm0() (+5 more)

### Community 11 - "Community 11"
Cohesion: 0.15
Nodes (14): AI Brain, MIDI Out, Visual Scenes (95+), Sidechain / Ducking (Pump), Terminal Themes, URL Sharing, WAV Export (Audio Recording), Ableton Live Integration (+6 more)

### Community 12 - "Community 12"
Cohesion: 0.24
Nodes (13): chord_root_pc(), degree_to_midi(), diatonic_chord(), is_degree(), Key, parse_chord(), parse_key_map(), parse_roman() (+5 more)

### Community 13 - "Community 13"
Cohesion: 0.25
Nodes (8): JsEvent, program_info_json(), ProgramInfo, Option, String, Vec, schedule_bar_json(), wave_name()

### Community 14 - "Community 14"
Cohesion: 0.20
Nodes (10): Pattern Syntax, Euclidean Rhythms, Probabilistic Notes, Ratchets & Rolls, Seed (Reproducible Randomness), Cellular Automata, Drums (Percussion), Melody (Notes) (+2 more)

### Community 15 - "Community 15"
Cohesion: 0.22
Nodes (9): Key & Tonality, Chords (Accords), Arpeggios, Gamelan (Indonesian Music), World Gammes (28+), Maqam (Arabic Music), Raga (Indian Music), Roman Numeral Chords (+1 more)

### Community 16 - "Community 16"
Cohesion: 0.29
Nodes (8): Distortion & Drive, Envelope (ADSR), Filters (LP/HP/BP/Notch), FM Synthesis, LFO (Low Frequency Oscillator), Reverb & Delay, Super-Saw (Unison), Waveforms

### Community 17 - "Community 17"
Cohesion: 0.36
Nodes (4): Accel, parse_accel(), Program, section_at_bar()

### Community 18 - "Community 18"
Cohesion: 0.25
Nodes (8): bare_uppercase_is_a_major_triad(), chord_expands_to_simultaneous_notes(), chord_respects_voice_octave(), oct_does_not_affect_absolute_notes(), per_voice_octave_shifts_degrees(), schedule_bar_keyed(), schedule_resolves_degrees_into_note_names(), uppercase_note_with_octave_stays_a_note()

### Community 19 - "Community 19"
Cohesion: 0.40
Nodes (6): degree_freq(), microtuning_matches_web_engine(), midi_to_freq(), note_to_freq(), tune_cents(), Tuning

### Community 20 - "Community 20"
Cohesion: 0.40
Nodes (5): Phase 1: Language, Phase 2.5: Sound Design & Presets, Phase 2: AI Brain & Verbs, Phase 3: Rust→WASM Engine, Phase 4: Terminal UI

### Community 21 - "Community 21"
Cohesion: 0.50
Nodes (3): InitInput, InitOutput, SyncInitInput

### Community 22 - "Community 22"
Cohesion: 0.67
Nodes (3): Section Tagging, Song Structure, Tempo & Meter

## Knowledge Gaps
- **101 isolated node(s):** `build-wasm.sh script`, `fs`, `path`, `ROOT`, `wasmPath` (+96 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **38 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `main()` connect `Music Theory & Architecture` to `Community 18`, `Scale & Arpeggio`?**
  _High betweenness centrality (0.072) - this node is a cross-community bridge._
- **Why does `parse_key()` connect `Scale & Arpeggio` to `Verb Operations`, `Music Theory & Architecture`, `Core Language Engine`, `Integration Tests`, `Community 12`, `Community 18`?**
  _High betweenness centrality (0.039) - this node is a cross-community bridge._
- **Why does `schedule_bar_keyed()` connect `Community 18` to `Verb Operations`, `Music Theory & Architecture`, `Core Language Engine`, `Scale & Arpeggio`, `Community 12`?**
  _High betweenness centrality (0.035) - this node is a cross-community bridge._
- **What connects `build-wasm.sh script`, `fs`, `path` to the rest of the system?**
  _102 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Verb Operations` be split into smaller, more focused modules?**
  _Cohesion score 0.09672619047619048 - nodes in this community are weakly interconnected._
- **Should `Music Theory & Architecture` be split into smaller, more focused modules?**
  _Cohesion score 0.05574912891986063 - nodes in this community are weakly interconnected._
- **Should `Core Language Engine` be split into smaller, more focused modules?**
  _Cohesion score 0.06116642958748222 - nodes in this community are weakly interconnected._