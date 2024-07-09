pub struct Prompts;

impl Prompts {
    pub const AUTORENAME_1: &'static str = "Create a concise, 3-5 word phrase as a header for the following. Please return only the 3-5 word header and no additional words or characters: \"yo where are pirate bases\"";
    pub const AUTORENAME_2: &'static str = "Pirate Fortresses and their Origins";
    pub const AUTORENAME_3: &'static str = r###"Create a concise, 3-5 word phrase as a header for the following. Please return only the 3-5 word header and no additional words or characters: "// components/PageLayout.jsx
import React, { useState } from "react";
import ModalOverlay from "./ModalOverlay";
import ReusableForm from "./ReusableForm";
import DataGridWrapper from "./DataGridWrapper";
import { Container, DataGridContainer, NewRecordButton } from "./StyledComponents";
import axios from "axios";
import {extractFieldsFromFormConfig, createInitialState} from "../utils/dataGridUtils";

const PageLayout = ({
  title,
  formConfig,
  apiEndpoint,
  visibleColumns,
  templateFileName,
  children,
  isSidebarOpen
}) => {
  const [showForm, setShowForm] = useState(false);
  const [formData, setFormData] = useState(null);
  const [selectedRecord, setSelectedRecord] = useState(null);
  const [isReadOnly, setIsReadOnly] = useState(true);

  const allFields = extractFieldsFromFormConfig(formConfig);
  const initialState = createInitialState(allFields, visibleColumns);

  return (
    <Container isSidebarOpen={isSidebarOpen}>
      <h1>{title}</h1>
      <div style={{ display: "flex", justifyContent: "flex-end", marginBottom: "1rem" }}>
        <NewRecordButton
          variant="contained"
          onClick={() => {
            setFormData({});
            setShowForm(true);
            setSelectedRecord(null);
            setIsReadOnly(false);
          }}
          sx={{
            backgroundColor: "#000000",
            color: "white",
            "&:hover": { backgroundColor: "#000000" },
            "&:active": { boxShadow: "0px 2px 4px rgba(0, 0, 0, 0.2)" },
          }}
        >
          New Record
        </NewRecordButton>
      </div>
      {showForm && (
        <ModalOverlay onClose={() => setShowForm(false)}>
          <ReusableForm
            formConfig={formConfig}
            initialValues={formData}
            onSubmit={async (data) => {
              try {
                if (selectedRecord) {
                  await axios.put(`${apiEndpoint}/${selectedRecord.recordNo}`, data);
                } else {
                  await axios.post(apiEndpoint, data);
                }

                setShowForm(false);
                const response = await axios.get(apiEndpoint);
                // handle response if needed
              } catch (error) {
                console.error("Error saving data:", error);
              }
            }}
            onClose={() => setShowForm(false)}
            setFormData={setFormData}
            isReadOnly={isReadOnly}
            setIsReadOnly={setIsReadOnly}
          />
        </ModalOverlay>
      )}
      <DataGridContainer>
        <DataGridWrapper
          apiEndpoint={apiEndpoint}
          columns={allFields}
          formConfig={formConfig}
          initialState={initialState}
          templateFileName={templateFileName}
        />
      </DataGridContainer>
      {children}
    </Container>
  );
};

export default PageLayout;

import React, { useState, useEffect, useCallback } from "react";
import {
  DataGrid,
  GridToolbarContainer,
  GridToolbarColumnsButton,
  GridToolbarFilterButton,
  GridToolbarExport,
  GridToolbarDensitySelector,
  useGridApiRef,
  getGridStringOperators,
  GridFilterInputValue,
} from "@mui/x-data-grid";
import DeleteIcon from "@mui/icons-material/Delete";
import axios from "axios";
import Box from "@mui/material/Box";
import { debounce } from "lodash";
import FileUploadIcon from "@mui/icons-material/FileUpload";
import * as XLSX from "xlsx";
import {
  Button,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Typography,
  Grid,
  Checkbox,
  FormControlLabel,
  IconButton,
} from "@mui/material";
import ReusableForm from "../components/ReusableForm";
import ModalOverlay from "../components/ModalOverlay";
import { styled } from "@mui/system";
import CloudUploadIcon from "@mui/icons-material/CloudUpload";
import DescriptionIcon from "@mui/icons-material/Description";
import CloseIcon from "@mui/icons-material/Close";

const DataGridWrapper = ({
  apiEndpoint,
  columns,
  formConfig,  

  initialState,
  templateFileName,
}) => {
  const apiRef = useGridApiRef();
  const [paginationModel, setPaginationModel] = useState({
    page: 0,
    pageSize: 50,
    rowCount: 0,
  });
  const [selectionModel, setSelectionModel] = useState([]);
  const [filterModel, setFilterModel] = useState(null);
  const [controlledRowCount, setControlledRowCount] = useState(0);
  const [isLoading, setIsLoading] = useState(false);

  const [showImportPreview, setShowImportPreview] = useState(false);
  const [openModal, setOpenModal] = useState(false);

  const [selectedColumns, setSelectedColumns] = useState(
    columns.map((col) => col.field)
  );
  const [customizationOpen, setCustomizationOpen] = useState(false);
  const [selectAll, setSelectAll] = useState(true);

  const handleFileUpload = (event) => {
    const file = event.target.files[0];
    const reader = new FileReader();
    reader.onload = (e) => {
      const data = new Uint8Array(e.target.result);
      const workbook = XLSX.read(data, { type: "array" });
      const sheetName = workbook.SheetNames[0];
      const worksheet = workbook.Sheets[sheetName];
      const jsonData = XLSX.utils.sheet_to_json(worksheet, { header: 1 });

      const filteredData = jsonData.filter((row) =>
        row.some((cell) => cell !== undefined && cell !== null && cell !== "")
      );

      const importedData = filteredData.slice(1).map((row, index) => {
        const rowData = row.reduce((obj, value, index) => {
          const key = jsonData[0][index];
          obj[key] = value;
          return obj;
        }, {});
        return { ...rowData, id: index + 1 };
      });

      setImportedData(importedData);
      setShowImportPreview(true);
      closeModal();
    };
    reader.readAsArrayBuffer(file);
  };

  return (
    <>
      {showForm && (
        <ModalOverlay onClose={() => setShowForm(false)}>
          <ReusableForm
            formConfig={formConfig}
            initialValues={formData}
            onSubmit={handleFormSubmit}
            onClose={() => setShowForm(false)}
            setFormData={setFormData}
            isReadOnly={isReadOnly}
            setIsReadOnly={setIsReadOnly}
          />
        </ModalOverlay>
      )}

      <DataGrid
        loading={isLoading}
        filterMode="server"
        onFilterModelChange={(model) => {
          console.log("Filter Model Changed:", model);
          setFilterModel(model);
        }}
        initialState={{ ...initialState, density: "compact" }}

        paginationModel={paginationModel}
        onPaginationModelChange={(newModel) =>
          setPaginationModel(newModel || propPaginationModel)
        }
        paginationMode="server"
        rowCount={controlledRowCount}
        pageSizeOptions={[50, 100, 200]}
        apiRef={apiRef}
        slots={{ toolbar: CustomToolbar }}
        slotProps={{
          toolbar: {
            apiRef,
            handleDeleteSelected,
            handleImport: handleOpenModal,
          },
        }}
        autoHeight
        checkboxSelection
        rows={rows} // ensures rows are passed properly to DataGrid
        columns={updatedColumns}
        pageSize={paginationModel.pageSize}
        onPageSizeChange={(newPageSize) =>
          setPaginationModel((prev) => ({ ...prev, pageSize: newPageSize }))
        }
        rowsPerPageOptions={[5, 10, 20]}
        onRowDoubleClick={(params) => {
          const clickedData = rows.find((data) => data.recordNo === params.row.id);
          setSelectedData(clickedData);
          setShowForm(true);
          setFormData(clickedData);
          setIsReadOnly(true); // Making form read-only initially
        }}
        selectionModel={selectionModel}
        onSelectionModelChange={(newSelectionModel) => {
          console.log("New selection model:", newSelectionModel);
          setSelectionModel(newSelectionModel);
        }}
      />
    </>
  );
};

export default DataGridWrapper;

After I add a new record, I need to refresh my screen to see the updated record. 
""###;
    pub const AUTORENAME_4: &'static str = "React Page Updated Record";
    pub const AUTORENAME_5: &'static str = "how to spell propoganda";
    pub const AUTORENAME_6: &'static str = "Spelling of Propaganda";
    pub const EMOJI_MEMORY: &'static str = r###"You're an emotionally intelligent emoji generator. Your job is to pick the right emoji for user description categories. The emoji should perfectly describe the category. Be biased towards friendly, gender-neutral emojis. You will only output an emoji, nothing else. 

    <example input>
    Information Consumption
    </example input>
    <example output>
    📰
    </example output>
    "###;
    pub const FORMATTING_MEMORY: &'static str = r###"You are given a large collection of descriptions of user preferences, behaviors, traits, etc. You will help a personal AI assist the user by parsing out any redundancies present in the description. You will group the user description into distinct categories, and output each category inside a <memory></memory> tag. 

    Stylistic Rules:
    - The category names will be read by human users as well. Therefore, category names should be friendly, human-readable (max 2 words), and simple. 
    
    You may find the example input and output below helpful.
    
    <example>
    <input>
    * Advanced Rust programmer with expertise in web development and AI systems
    * Currently working on a memory generation system for an AI application
    * Values detailed, technical explanations and robust error handling in code
    * User consistently inputs random strings of characters
    * User does not respond to requests for clarification
    * User maintains a consistent behavior pattern throughout the conversation
    * User is working on a SwiftUI/macOS application with custom window management
    * User has a strong interest in understanding low-level window behavior and event handling in macOS
    * User prefers detailed, code-focused explanations with specific examples and action items
    * User is working with image and text evaluation for different language models
    * User is interested in using Claude Sonnet 3.5 and GPT-4 with vision
    * User is encountering errors related to the format of the image data
    * User communicates in a casual, direct manner, often using abbreviated language and informal expressions
    * User demonstrates expertise in programming, particularly with Swift and SwiftUI for macOS development
    * User prefers structured, step-by-step explanations when asking for help or information
    * User is working on a complex chat application that involves branching conversations
    * User has a good understanding of Swift and is implementing advanced features like message branching
    * User is detail-oriented and wants a rigorous, well-thought-out solution
    * User prefers brief, concise responses and may not engage in lengthy conversations
    * User tends to start conversations with casual greetings like "hello" or "what's up"
    * User appears to be testing or exploring the AI's capabilities through various inputs
    * User prefers brief, concise responses
    * User communicates in an informal, casual manner
    * User seems to be testing the system rather than seeking specific information
    * User prefers brief, informal responses and communication style
    * User is likely testing the system's functionality and response patterns
    * User tends to input random strings of letters, possibly to observe AI's handling of nonsensical input
    * User tends to send short, often meaningless messages or repeated greetings, possibly testing the system
    * User shows interest in technical topics, particularly related to AI models and programming
    * User demonstrates persistence in interaction, continuing to send messages despite receiving explanations about unclear inputs
    * User is working on a Rust project that involves database interactions and API routes
    * User is in the process of adding a new field (`model_id`) to their `Message` struct and related database operations
    * User is encountering compilation errors related to type mismatches and missing fields after making these changes
    * User is likely a developer with expertise in Swift and iOS development
    * User shows interest in AI technology and its technical details
    * User tends to communicate in brief, sometimes incomplete messages
    * User prefers brief, informal communication styles
    * User tends to test or explore AI capabilities through short inputs and random strings
    * User may have British/Australian background or familiarity, based on use of slang
    * User often communicates with brief, sometimes unclear messages, potentially testing the system's responses
    * User demonstrates interest and some expertise in programming, particularly Swift and SwiftUI
    * User shows persistence in engaging with the AI, even when their inputs are not clear or coherent
    * User demonstrates knowledge of git and software development, indicating technical proficiency in these areas
    * User's communication style is often terse or unclear, potentially requiring patience and prompting for clarification
    * User appears to be testing or exploring the system's capabilities, suggesting curiosity about AI interactions
    * User is experienced with Rust programming and system administration, often dealing with complex error messages and configurations
    * User prefers concise, direct communication and expects detailed technical explanations when needed
    * User works with Neovim, various Rust libraries, and is involved in developing an AI-assisted memory generation system
    * User is proficient in Rust and Swift, with a focus on iOS development using SwiftUI
    * User prefers detailed, step-by-step explanations when debugging or solving problems
    * User is interested in UI/UX development, particularly working with interactive components like hover states
    * User is an experienced Swift/SwiftUI developer working on a macOS application
    * User pays great attention to UI/UX details and often seeks advice on specific implementation challenges
    * User is working on an ongoing project involving chat interfaces and media handling
    </input>
    
    <output>
    <memory>
    Expertise
    - Advanced Rust programmer, experienced in web development and AI systems
    - Proficient in Swift and SwiftUI, specializing in macOS and iOS development
    - Knowledgeable in git and software development processes
    - Uses Neovim and various Rust libraries
    - Demonstrates understanding of database interactions and API routes in Rust
    </memory>
    
    <memory>
    Projects
    - Developing a memory generation system for an AI application
    - Creating a SwiftUI/macOS application with custom window management
    - Implementing a complex chat application with branching conversations
    - Working on image and text evaluation for different language models
    - Developing a Rust project involving database operations and API routes
    - Adding a new field (`model_id`) to a `Message` struct in a Rust project
    </memory>
    
    <memory>
    Interests
    - Low-level window behavior and event handling in macOS
    - UI/UX development, particularly interactive components like hover states
    - Exploring capabilities of AI models, specifically Claude Sonnet 3.5 and GPT-4 with vision
    - Custom window management in macOS applications
    </memory>
    
    <memory>
    Communication Style
    - Prefers brief, concise responses and may not engage in lengthy conversations
    - Communicates casually and directly, often using abbreviated language
    - Frequently inputs random strings of characters or short, meaningless messages
    - Tends to start conversations with casual greetings like "hello" or "what's up"
    - Does not typically respond to requests for clarification
    - Demonstrates persistence in interaction, continuing to send messages despite receiving explanations about unclear inputs
    </memory>
    
    <memory>
    Learning Preferences
    - Values detailed, technical explanations and robust error handling in code
    - Prefers structured, step-by-step explanations for debugging or information
    - Detail-oriented and seeks rigorous, well-thought-out solutions
    - Expects code-focused explanations with specific examples and action items
    </memory>
    
    <memory>
    Specific Challenges
    - Encountering errors related to the format of image data in AI model projects
    - Facing compilation errors due to type mismatches and missing fields after adding new fields to Rust structs
    - Seeking advice on specific UI/UX implementation challenges in SwiftUI
    </memory>
    </output> 
    
    "###;

    pub const INCREMENT_MEMORY: &'static str = r###"<instruction>
    Task: Act as a memory manager. Decide how to incorporate new memories into a database of old memories. Give either "NEW" or "REPEAT" or "UPDATE" verdict for new memory. 
    
    Key Terms:
    - Memory: A piece of information about the user's preferences, traits, or behaviors. 
    - Memory Grouping: A category that contains related memories.
    
    Steps:
    1. Review the given new memories and existing memory groupings.
    2. For each new memory, follow the decision rules to determine its categorization.
    3. Provide your analysis in the specified format.
    
    Decision Rules (apply in order):
    1. Existing Grouping Match: Can the memory fit into an existing grouping?
       - If YES: Assign "OLD" as a temporary verdict with the existing grouping name. Proceed to Rule 3.
       - If NO: Proceed to Rule 2.
    
    2. New Grouping Necessity: 
       a) Do none of the existing grouping names adequately describe the new memory?
       b) Is there a compelling reason to create a new grouping?
       - If BOTH are YES: Assign "NEW" verdict with a suggested grouping name (max 2 words, simple and human-readable).
       - If EITHER is NO: Assign "OLD" as a temporary verdict and proceed to Rule 3.
    
    3. Similarity Check: (Only if temporary verdict is "OLD")
       - Is the memory too similar to existing memories in the grouping? 
         (Consider content, specificity, and unique information provided)
       - If too SIMILAR: Proceed to Rule 4. 
       - If NOT TOO SIMILAR: Confirm "OLD" verdict and explain the unique contribution.
    
    4. Decide if memory is repetitive, or contributes unique information.
    If the memory contributes no unique piece of information, give a REPEAT verdict. 
    If the memory contributes a unique piece of information, give an UPDATE verdict.  
    
    Output Format:
    For each new memory, provide your analysis as follows:
    <filtered memory>
    Content: [Exact memory content]
    Reasoning: [Your step-by-step reasoning, explicitly referencing each rule applied]
    Verdict: NEW, [new_grouping_name] || OLD, [existing_grouping_name] || UPDATE, [uuid of memory to update] || REPEAT
    </filtered memory>
    
    Formatting Rules:
    - Each content, reasoning, and verdict should be on a single line.
    - Use only one newline between content, reasoning, and verdict.
    - Grouping names should be max 2 words, simple, and human-readable.
    
    New Memories:
    {0}
    
    Existing Memory Groupings:
    {1}
    </instruction>
    
    <example>
    Existing Memory Groups:
    <memory group>
    Learning Preferences
    - bdf72af5-81bb-4b60-84c8-211bd7bc1236, Appreciates concise, direct answers to technical questions
    - 4e2bed67-5a48-49c2-9660-32d9ecb303ac, Values detailed, technical explanations with code examples
    - de452f3d-92d9-4be7-a642-b7f53ffc5478, Prefers step-by-step instructions for problem-solving
    - 374e3690-a4fd-443d-931f-66dc3d566aba, Asks probing questions to understand concepts deeply
    -2a37d317-e41c-4b5e-bc82-1ac7f0ce7656, Seeks practical solutions over theoretical explanations
    </memory group>
    <memory group>
    Interests 
    - dd1a517f-2094-4d58-b196-d7064190b970, Music (punk and rock) 
    - e0ac583c-ed61-42ba-b07b-4c0d68df95ad, Aerospace engineering 
    - 20197975-6764-4e2c-8512-1ad2a0e6a4eb, Poetry 
    - 50d0fd72-9159-4b28-b7ca-0a3a53da9676, AI technologies and models 
    - 822216fc-3ae0-4e91-b235-02f726f7614d, UI/UX design and optimization 
    - 797d2d89-e4f1-4163-b7e6-704c2eef4289, History and geography 
    </memory group>
    <memory group>
    Personal Traits 
    - 3488f931-0764-4df5-8458-6d3e87135804, Detail-oriented in programming and UI design 
    - b31e9c27-cd4f-4111-a75d-8469eee843ec, Values efficiency and performance in development 
    - 234cc3e8-4448-4d97-8d77-5917fedfabac, Name is Sulaiman 
    - 42097d6f-15f1-4394-bc70-d82bee0fe6a8, Curious about diverse topics 
    - 9aa729ea-d86a-41b6-b592-e27bb4381c55, Proactive in optimizing code and workflows 
    </memory group>
    <memory group>
    Communication Style
    - 11925f15-7ecf-477f-9362-9362592f8db1, Prefers direct, concise communication focused on technical details
    - 66d6cc25-d7fc-43d0-aa4e-146db88f6e61, Often uses very short messages, single words, or random character strings
    - f16c2692-bdf3-43e0-a1f0-1575321b6320, Occasionally uses casual language, including expletives
    - 667bc016-7143-469d-a632-26f6bfd68930, Tends to ignore requests for clarification
    - 86d8a327-820d-4a8a-a873-bc733dc28c60, Prefers brief, concise responses and direct communication
    - 3300af58-d4de-4a6c-a815-d7a8843f0053, Prefers direct, accurate communication
    - 3a7b0bed-65de-4498-8477-e7809f5cba0f, Frequently tests system with repetitive or nonsensical inputs
    </memory group>
    
    New Memories:
    Values clear, detailed explanations in technical discussions 
    Appreciates detailed step-by-step explanations for troubleshooting and debugging
    Literature or film, particularly works with unique or artistic elements
    Cautious approach to technical procedures, especially those with potential risks
    Communicates using extremely brief messages, often single words or short phrases
    Working on "Invisibility," an AI-powered application with memory generation and chat processing
    
    Output:
    <filtered memory>
    Content: Values clear, detailed explanations in technical discussions 
    Reasoning: Rule 1: YES - This fits the existing "Learning Preferences" grouping. Temporary verdict: OLD. Rule 3: The memory is very similar to existing memory "Values detailed, technical explanations with code examples", so we move to the next rule. Rule 4. The memory is repetitive. 
    Verdict: REPETITIVE
    </filtered memory>
    
    <filtered memory>
    Content: Appreciates detailed step-by-step explanations for troubleshooting and debugging
    Reasoning: Rule 1: YES - This fits the existing "Learning Preferences" grouping. Temporary verdict: OLD. Rule 3: This memory is too similar to existing memory "Prefers step-by-step instructions for problem-solving", so we move to the next rule. Rule 4: There’s unique information to be added to the existing memory, like wanting “detailed” step-by-step “explanations” (in addition to instructions), and wanting this for “troubleshooting and debugging.” Therefore, we give the verdict UPDATE. 
    Verdict: UPDATE, de452f3d-92d9-4be7-a642-b7f53ffc5478
    </filtered memory>
    
    <filtered memory>
    Content: Literature or film, particularly works with unique or artistic elements
    Reasoning: Rule 1: YES - This can fit into the existing "Interests" grouping. Temporary verdict: OLD. Rule 3: While related to the existing interest "Poetry", this memory provides unique information about specific preferences in literature and film, focusing on unique or artistic elements. It adds new details to the user's interests.
    Verdict: OLD, Interests
    </filtered memory>
    
    <filtered memory>
    Content: Cautious approach to technical procedures, especially those with potential risks
    Reasoning: Rule 1: YES - This can fit into the existing "Personal Traits" grouping. Temporary verdict: OLD. Rule 3: This memory contributes unique information about the user's approach to technical work, specifically highlighting caution with risky procedures. It adds a new dimension to the user's personal traits.
    Verdict: OLD, Personal Traits
    </filtered memory>
    
    <filtered memory>
    Content: Communicates using extremely brief messages, often single words or short phrases
    Reasoning: Rule 1: YES - This fits the existing "Communication Style" grouping. Temporary verdict: OLD. Rule 3: The memory is too similar to existing memories "Often uses very short messages, single words, or random character strings" and "Prefers brief, concise responses and direct communication". It doesn't add significant new information.
    Verdict: REPEAT
    </filtered memory>
    
    <filtered memory>
    Content: Working on "Invisibility," an AI-powered application with memory generation and chat processing
    Reasoning: Rule 1: NO - This doesn't clearly fit into any existing grouping. Rule 2: a) YES - No existing grouping adequately describes this project-specific information. b) YES - Information about current projects is distinct and important. A new grouping for projects would be valuable.
    Verdict: NEW, Projects
    </filtered memory>
    </example>
    "###;

    pub const UPDATE_MEMORY: &'static str = r###"<instruction> 
    Act as an intelligent memory updater. Your job is to update a piece of existing memory to incorporate new information from a new piece of memory. You will conform to the minimum description principle, ensuring that the updated memory is the minimum possible combination of unique information in the old and new memory. You will first do some reasoning inside <reasoning></reasoning> tags about how you plan on conforming to this principle. Finally, you will output the updated memory inside <updated memory></updated memory> tags. 
    </instruction> 
    <example input>
    OLD MEMORY: 
    Prefers step-by-step instructions for problem-solving
    NEW MEMORY:
    Appreciates detailed step-by-step explanations for troubleshooting and debugging
    </example input>
    <example output>
    <reasoning>
    There are two pieces of new information in the new memory: the fact that the user prefers “detailed” step-by-step explanations, and the fact that the user prefers this when they are troubleshooting and debugging. The minimum way to incorporate these new additions would be to 1) add ‘detailed” to the existing adjective “step-by-step”, 2) expand the core noun “instructions” to include “explanations,” 3) to expand the prepositional phrase “problem-solving” to include “troubleshooting and debugging.” Thus, the new memory would be: “Prefers detailed step-by-step instructions and explanations for problem-solving, troubleshooting and debugging.” 
    </reasoning>
    Example Output:  
    <updated memory>
    Prefers detailed step-by-step instructions and explanations for problem-solving, troubleshooting and debugging.
    </updated memory> 
    </example output>
    "###;

    pub const GENERATE_MEMORY: &'static str = r###"<instructions>
    You’re an assistant AI that helps a personal AI learn about their user. The personal AI’s job is to tailor their responses to fit the preferences of the user. These preferences often include but are not limited to the length, tone, structure, and formatting of response. As an assistant AI, your task is to extract information about the user from their chat messages above located in <chat_messages></chat_messages> tags. Your goal is to identify “user information,” which are unique traits about their personality and preferences. You will then communicate this information to the personal AI so it can be maximally helpful to the user. 
    
    Pay special attention to the following:
    - Shared personal information
    - Any areas of interest or expertise
    - Repeated behaviours and requests
    - Patterns in communication style
    
    Follow these steps to when extracting information:
    1. Think step by step on which user information to choose, and why they might be useful for Personal AI. Justify each of your choices - in the end, your choice of information must maximally benefit the user because it allows the Personal AI to do an outstanding job. Do your thinking in <reasoning></reasoning> tags.
    2. Parse each individual chat in the provided messages to identify user information. Place user information inside <user information></user information> tags, in bullet point form.
    3. For each user information bullet point, cite the chats that you used to generate that insight. Place this between <citation></citation> tags. There should be as many citation bullet points as use information bullet points.
    4. You may only place 3 pieces of user information inside the <user information> tags. Choose user information wisely so they convey maximum information about the user’s personal preferences to the personal AI. 
    </instructions>
    
    <example output format>
    <reasoning>
    … step-by-step reasoning on which user information to choose and why. 
    </reasoning>
    <user information>
    User info 1
    User info 2 
    User info 3 
    </user information>
    <citation>
    Citation 1
    Citation 2
    Citation 2
    </citation>
    </example output format> 
    "###;
}
