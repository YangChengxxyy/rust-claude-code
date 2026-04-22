## ADDED Requirements

### Requirement: NotebookEditTool supports cell-oriented edits
The tool system SHALL provide a notebook editing tool that performs structured edits on Jupyter notebook files at the cell level.

#### Scenario: Replace an existing cell
- **WHEN** the model invokes the notebook editing tool to replace a specific cell in an `.ipynb` file
- **THEN** the tool updates only the targeted cell and returns a success result describing the applied change

#### Scenario: Insert a new cell
- **WHEN** the model invokes the notebook editing tool to insert a new cell at a valid position
- **THEN** the tool writes the updated notebook structure and reports the inserted cell index

### Requirement: NotebookEditTool validates notebook structure
The notebook editing tool SHALL reject edits that would produce an invalid notebook document.

#### Scenario: Input file is not a valid notebook
- **WHEN** the tool is invoked on a file that cannot be parsed as a Jupyter notebook
- **THEN** it returns an error result and does not modify the file

#### Scenario: Edit request targets an invalid cell index
- **WHEN** the tool is asked to edit or delete a cell index that does not exist
- **THEN** it returns an error result and leaves the notebook unchanged
