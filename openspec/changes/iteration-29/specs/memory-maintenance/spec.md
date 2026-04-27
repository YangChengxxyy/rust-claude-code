## ADDED Requirements

### Requirement: The memory maintenance workflow SHALL support automated extraction flows
The maintenance workflow SHALL accept memory write requests produced by automatic extraction and process them through the same topic-file-first model used by manual memory writes.

#### Scenario: Automated extraction saves project memory
- **WHEN** automatic extraction produces a durable `project` memory candidate
- **THEN** the system writes the candidate to a topic file and updates `MEMORY.md`

#### Scenario: Automated extraction corrects feedback memory
- **WHEN** automatic extraction produces a `feedback` memory candidate that duplicates existing feedback
- **THEN** the system updates the existing topic file and rebuilds the index

### Requirement: Automated memory maintenance SHALL be best-effort
Automatic memory maintenance failures SHALL NOT fail the user-facing response that triggered the memory candidate.

#### Scenario: Auto-memory write fails
- **WHEN** automatic memory maintenance cannot write a candidate because of an I/O error
- **THEN** the system reports or logs the memory write failure while allowing the assistant response to complete

#### Scenario: Auto-memory index rebuild fails
- **WHEN** a topic file write succeeds but index rebuild fails
- **THEN** the system reports or logs the index rebuild failure and leaves the topic file in place
