# Changelog

All notable changes to this project will be documented in this file.

## [0.7.0] - 2025-07-27

### 🐛 Bug Fixes

- Fix tests

### 🚜 Refactor

- Add ability to provide workflow
- Refactoring sql

### 📚 Documentation

- Docks
- Docs

### 🧪 Testing

- Tests
- Test getting drift metric values
- Testing archive logic
- Testing llm drift worker
- Testing LLMRecord
- Tests
- Testing out arcs

## [0.6.5] - 2025-07-17

### 🚜 Refactor

- Should be able to instantiate Feature without specifying type
- Parsing logic for list and dictionary
- Switching over to batch insert

### 📚 Documentation

- Docs

## [0.5.6] - 2025-06-17

### 🐛 Bug Fixes

- Remove option for psi threshold in config

### 🧪 Testing

- Update tests for default PsiThreshold
- Fix populate script

## [0.5.5] - 2025-06-16

### 🐛 Bug Fixes

- Fix tests
- Fix lints
- Add test for string and no categorical

### 🚜 Refactor

- *psi_threshold* Threshold_config -> threshold

### 📚 Documentation

- Adding changelog
- Add threshold types
- Add script to generate api specs
- Add make command for changelog

## [0.5.4] - 2025-06-14

* add templates by @thorrester in https://github.com/demml/scouter/pull/77
* task manager by @thorrester in https://github.com/demml/scouter/pull/91
* Fix data profiling feature bug by @thorrester in https://github.com/demml/scouter/pull/92

## [0.5.2] - 2025-05-31

* Feature/update for opsml by @thorrester in https://github.com/demml/scouter/pull/76

## [0.5.1] - 2025-05-28

* updates for testing purposes by @thorrester in https://github.com/demml/scouter/pull/75

## [0.5.0] - 2025-05-17

* test fix by @russellkemmit in https://github.com/demml/scouter/pull/68
* add flume channel queue by @russellkemmit in https://github.com/demml/scouter/pull/70
* Feature/redis by @thorrester in https://github.com/demml/scouter/pull/66
* Refactor/archive sql trx by @thorrester in https://github.com/demml/scouter/pull/71
* Odds and ends for opsml compatibility + Error re-write by @thorrester in https://github.com/demml/scouter/pull/69
* Refactor/drift polling by @thorrester in https://github.com/demml/scouter/pull/72
* Feature/sql optimization by @thorrester in https://github.com/demml/scouter/pull/73
* Bump version by @thorrester in https://github.com/demml/scouter/pull/74


## [0.4.6] - 2025-05-05

* updates per iris integration by @russellkemmit in https://github.com/demml/scouter/pull/53
* remove targets by @russellkemmit in https://github.com/demml/scouter/pull/56
* Feature add long term storage by @thorrester in https://github.com/demml/scouter/pull/57
* Opsml UI by @thorrester in https://github.com/demml/scouter/pull/58
* Feature/long term storage continued by @thorrester in https://github.com/demml/scouter/pull/59
* Update/cleanup error handling by @thorrester in https://github.com/demml/scouter/pull/60
* Feature/longterm storage cloud by @thorrester in https://github.com/demml/scouter/pull/62
* Feature/long term storage #3 by @thorrester in https://github.com/demml/scouter/pull/61
* Update/readme by @thorrester in https://github.com/demml/scouter/pull/63
* Update ts-component-data-archive.md by @thorrester in https://github.com/demml/scouter/pull/64
* Refactor/api integration by @thorrester in https://github.com/demml/scouter/pull/65
* bump version number by @russellkemmit in https://github.com/demml/scouter/pull/67


## [0.4.5] - 2025-02-05

* Update release.yml by @thorrester in https://github.com/demml/scouter/pull/51

## [0.4.4] - 2025-02-04

* Updates by @thorrester in https://github.com/demml/scouter/pull/47
* Update release.yml by @thorrester in https://github.com/demml/scouter/pull/46
* Adding metrics + refactoring consumers by @thorrester in https://github.com/demml/scouter/pull/49

## [0.4.3] - 2025-01-31

* Add Custom Queue / add tracing / fix postgres bin issue / Fastapi integration for multiple profiles (4 PRs in 1 :) )


## [0.4.2] - 2025-01-29

* Merge pull request #43 from demml/fixcomposetype
* update migration schema

## [0.4.1] - 2025-01-26

* Fix release
* Update data profiler
* Update build assets workflow to create static bins
* Feature/refactor drift profiles

## [0.4.0] - 2025-01-06

* Create custom drift profile
* Update deps

## [0.3.3] - 2024-11-28

* AddMonitoringContextManager
* Add PSI logic
* Psi alerts