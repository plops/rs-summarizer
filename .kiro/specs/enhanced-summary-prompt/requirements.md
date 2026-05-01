# Requirements Document

## Introduction

This feature upgrades the summary generation prompt in rs-summarizer to match the Python program's (source04/tsum/p04_host.py) approach. The current minimal system prompt and simple user prompt are replaced with a detailed "adaptive knowledge synthesis engine" system instruction, a few-shot example (example transcript + expected output), and a structured user prompt template. All prompt content is compiled into the binary via `include_str!` for consistency and reproducibility of data collection.

## Glossary

- **Summary_Service**: The Rust service (`src/services/summary.rs`) responsible for building prompts and invoking the Gemini API to generate summaries.
- **System_Instruction**: The detailed persona prompt ("adaptive knowledge synthesis engine") sent as a system-level instruction to Gemini models that support it.
- **User_Prompt**: The message sent as user content to the Gemini API, containing the few-shot example and the real transcript.
- **Few_Shot_Example**: A demonstration consisting of an example input transcript (with title, description, comments) and corresponding expected outputs (abstract + bullet-point summary).
- **Gemma_Model**: A model whose name starts with "gemma", which does not support system instructions via the API.
- **Gemini_Model**: A model whose name does not start with "gemma", which supports system instructions as a separate parameter.
- **Prompt_Assets**: The static text files (system instruction, example input, example output, example output abstract) compiled into the binary.

## Requirements

### Requirement 1: Compile Prompt Assets into Binary

**User Story:** As a developer, I want all prompt text assets compiled into the binary at build time, so that the prompts are immutable at runtime and produce consistent data for collection.

#### Acceptance Criteria

1. THE Summary_Service SHALL include the system instruction text as a compile-time constant using `include_str!`
2. THE Summary_Service SHALL include the example input text as a compile-time constant using `include_str!`
3. THE Summary_Service SHALL include the example output text as a compile-time constant using `include_str!`
4. THE Summary_Service SHALL include the example output abstract text as a compile-time constant using `include_str!`
5. THE Summary_Service SHALL NOT provide any mechanism to override or modify prompt content at runtime

### Requirement 2: System Instruction for Gemini Models

**User Story:** As a developer, I want Gemini models to receive the detailed "adaptive knowledge synthesis engine" system instruction, so that summaries are generated with domain-expert quality matching the Python program's output.

#### Acceptance Criteria

1. WHILE the selected model is a Gemini_Model, THE Summary_Service SHALL send the System_Instruction as the system prompt parameter to the Gemini API
2. THE System_Instruction SHALL contain the identical text used by the Python program's `generate_and_save` function (the "CORE INSTRUCTION" and "PROCESS PROTOCOL" persona prompt)
3. WHILE the selected model is a Gemini_Model, THE Summary_Service SHALL NOT include the System_Instruction text in the User_Prompt

### Requirement 3: System Instruction Injection for Gemma Models

**User Story:** As a developer, I want Gemma models to receive the system instruction as part of the user prompt, so that they benefit from the same persona guidance despite lacking system instruction support.

#### Acceptance Criteria

1. WHILE the selected model is a Gemma_Model, THE Summary_Service SHALL prepend the System_Instruction text to the User_Prompt
2. WHILE the selected model is a Gemma_Model, THE Summary_Service SHALL NOT pass a system prompt parameter to the Gemini API
3. WHILE the selected model is a Gemma_Model, THE Summary_Service SHALL separate the prepended System_Instruction from the rest of the User_Prompt with a clear delimiter

### Requirement 4: Few-Shot Example in User Prompt

**User Story:** As a developer, I want the user prompt to include one complete example (input transcript and expected output), so that the model has a concrete demonstration of the desired summary format.

#### Acceptance Criteria

1. THE User_Prompt SHALL contain the full example input text before the real transcript
2. THE User_Prompt SHALL contain the example output abstract text as part of the expected output demonstration
3. THE User_Prompt SHALL contain the example output text (bullet-point summary) as part of the expected output demonstration
4. THE User_Prompt SHALL present the example in the order: example input, then example output abstract, then example output summary
5. THE User_Prompt SHALL clearly label the example input section and example output section with text markers

### Requirement 5: User Prompt Template Structure

**User Story:** As a developer, I want the user prompt to follow the same template structure as the Python program's `get_prompt` function, so that the Rust and Python programs produce equivalent API requests.

#### Acceptance Criteria

1. THE User_Prompt SHALL begin with an instruction paragraph explaining that an example and then a real transcript will follow
2. THE User_Prompt SHALL include a bold instruction requesting an abstract and bullet-list summary with timestamps
3. THE User_Prompt SHALL place the real transcript after the few-shot example section
4. THE User_Prompt SHALL include the "What would be a good group of people to review this topic?" framing before the real transcript
5. THE User_Prompt SHALL use the same textual structure and markers as the Python `get_prompt` function

### Requirement 6: Backward Compatibility of API Interaction

**User Story:** As a developer, I want the enhanced prompt to integrate with the existing streaming summary generation flow, so that no changes are needed to the streaming, DB persistence, or cost computation logic.

#### Acceptance Criteria

1. THE Summary_Service SHALL continue to use the existing streaming response processing after sending the enhanced prompt
2. THE Summary_Service SHALL continue to persist chunks to the database progressively during streaming
3. THE Summary_Service SHALL continue to compute costs using the existing token-based formula
4. WHEN the enhanced prompt is sent, THE Summary_Service SHALL continue to handle rate-limit errors identically to the current implementation

### Requirement 7: Prompt Content Fidelity

**User Story:** As a developer, I want the prompt content to be byte-for-byte identical to the Python program's prompt files, so that both programs produce comparable summaries for data consistency.

#### Acceptance Criteria

1. THE Prompt_Assets for example input SHALL be copied from `source04/tsum/example_input.txt` without modification
2. THE Prompt_Assets for example output SHALL be copied from `source04/tsum/example_output.txt` without modification
3. THE Prompt_Assets for example output abstract SHALL be copied from `source04/tsum/example_output_abstract.txt` without modification
4. THE Prompt_Assets for the system instruction SHALL reproduce the exact text from the Python program's `generate_and_save` function `system_instruction` parameter
