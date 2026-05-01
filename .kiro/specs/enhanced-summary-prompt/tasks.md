# Implementation Plan: Enhanced Summary Prompt

## Overview

Replace the minimal prompt in `SummaryService` with the full prompt structure from the Python program. This involves creating prompt asset files, adding `include_str!` constants, refactoring `build_prompt()` to use the few-shot template, adding Gemma-specific prompt injection, and validating with property-based and unit tests.

## Tasks

- [x] 1. Create prompt asset files
  - [x] 1.1 Create `prompts/system_instruction.txt`
    - Extract the system instruction text from the Python `generate_and_save` function in `source04/tsum/p04_host.py`
    - This is the "adaptive knowledge synthesis engine" persona prompt (the `system_instruction=` parameter value)
    - Place in `rs-summarizer/prompts/system_instruction.txt`
    - _Requirements: 1.1, 7.4_
  - [x] 1.2 Create `prompts/example_input.txt`
    - Copy `source04/tsum/example_input.txt` byte-for-byte to `rs-summarizer/prompts/example_input.txt`
    - _Requirements: 1.2, 7.1_
  - [x] 1.3 Create `prompts/example_output.txt`
    - Copy `source04/tsum/example_output.txt` byte-for-byte to `rs-summarizer/prompts/example_output.txt`
    - _Requirements: 1.3, 7.2_
  - [x] 1.4 Create `prompts/example_output_abstract.txt`
    - Copy `source04/tsum/example_output_abstract.txt` byte-for-byte to `rs-summarizer/prompts/example_output_abstract.txt`
    - _Requirements: 1.4, 7.3_

- [x] 2. Add include_str! constants and refactor build_prompt
  - [x] 2.1 Add `include_str!` constants to `src/services/summary.rs`
    - Add `SYSTEM_INSTRUCTION`, `EXAMPLE_INPUT`, `EXAMPLE_OUTPUT_ABSTRACT`, and `EXAMPLE_OUTPUT` constants at module level
    - Use relative paths `../../prompts/<filename>` from the file location
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_
  - [x] 2.2 Refactor `build_prompt()` to use the Python `get_prompt()` template
    - Replace the current simple prompt with the full template structure:
      - Instruction paragraph explaining the example/real-transcript flow
      - Bold formatting instruction requesting abstract + bullet-list summary
      - "Example Input:" marker followed by `EXAMPLE_INPUT`
      - "Example Output:" marker followed by `EXAMPLE_OUTPUT_ABSTRACT` and `EXAMPLE_OUTPUT`
      - "Here is the real transcript" framing with the actual transcript
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 5.1, 5.2, 5.3, 5.4, 5.5_
  - [x] 2.3 Add `build_prompt_for_gemma()` helper method
    - Prepend `SYSTEM_INSTRUCTION` to the output of `build_prompt()` with `\n\n---\n\n` delimiter
    - _Requirements: 3.1, 3.3_

- [x] 3. Update generate_summary model routing
  - [x] 3.1 Update `generate_summary()` to use `SYSTEM_INSTRUCTION` constant for Gemini models
    - Replace the hardcoded system prompt string with the `SYSTEM_INSTRUCTION` constant
    - _Requirements: 2.1, 2.2_
  - [x] 3.2 Route Gemma models to `build_prompt_for_gemma()`
    - When model name starts with "gemma", use `build_prompt_for_gemma()` and skip the system prompt parameter
    - When model name does not start with "gemma", use `build_prompt()` and pass `SYSTEM_INSTRUCTION` as system prompt
    - _Requirements: 2.3, 3.1, 3.2_

- [x] 4. Checkpoint - Verify build compiles
  - Ensure `cargo build` succeeds with the new `include_str!` paths resolving correctly, ask the user if questions arise.

- [ ] 5. Add property-based tests
  - [ ]* 5.1 Write property test for template structure (Property 1)
    - **Property 1: Prompt template structure**
    - Generate random transcript strings (empty, unicode, long) using proptest
    - Verify `build_prompt(transcript)` contains instruction paragraph, bold instruction, "Example Input:" marker, example input content, "Example Output:" marker, example abstract, example output, "Here is the real transcript" framing, and the transcript — in that order
    - Minimum 100 iterations
    - **Validates: Requirements 4.1, 4.2, 4.3, 4.4, 4.5, 5.1, 5.2, 5.3, 5.4**
  - [ ]* 5.2 Write property test for Gemini model routing (Property 2)
    - **Property 2: Gemini model prompt routing**
    - Generate random non-gemma model name strings and transcripts
    - Verify the user prompt from `build_prompt()` does NOT contain the system instruction text
    - Minimum 100 iterations
    - **Validates: Requirements 2.1, 2.3**
  - [ ]* 5.3 Write property test for Gemma model injection (Property 3)
    - **Property 3: Gemma model prompt injection**
    - Generate random gemma-prefixed model names and transcripts
    - Verify `build_prompt_for_gemma(transcript)` starts with the system instruction, contains the `---` delimiter, and the template follows after the delimiter
    - Minimum 100 iterations
    - **Validates: Requirements 3.1, 3.2, 3.3**

- [x] 6. Update unit tests
  - [x] 6.1 Add unit tests for prompt asset constants
    - Verify `SYSTEM_INSTRUCTION` is non-empty and contains "CORE INSTRUCTION"
    - Verify `EXAMPLE_INPUT` is non-empty and contains expected content (e.g., "Fluidigm Polaris")
    - Verify `EXAMPLE_OUTPUT` is non-empty
    - Verify `EXAMPLE_OUTPUT_ABSTRACT` is non-empty and contains "Abstract"
    - _Requirements: 1.1, 1.2, 1.3, 1.4_
  - [x] 6.2 Update existing `test_build_prompt_contains_transcript` and `test_build_prompt_non_empty` tests
    - Adjust assertions to match the new template structure (the prompt now contains the few-shot example)
    - Verify the transcript is still present in the output
    - Verify the "Example Input:" and "Example Output:" markers are present
    - _Requirements: 4.5, 5.3_
  - [x] 6.3 Add unit test for `build_prompt_for_gemma`
    - Verify the output starts with system instruction content
    - Verify the `---` delimiter separates system instruction from the template
    - Verify the template portion matches `build_prompt()` output
    - _Requirements: 3.1, 3.3_

- [x] 7. Final checkpoint - Ensure all tests pass
  - Run `cargo test` and ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- The design uses Rust with `proptest` (already in dev-dependencies)
- All prompt assets are compiled into the binary — no runtime file I/O
- The streaming, DB persistence, and cost computation logic remain unchanged (Requirement 6)
- Each task references specific requirements for traceability
- Property tests validate universal correctness properties from the design document
