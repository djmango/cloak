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
    pub const EMOJI_MEMORY: &'static str = r###"You're an emoji generator. Your job is to generate an appropriate emoji based on descriptions about the user. The emoji should allow the user to easily identify what the entire description would say. You will only output an emoji, nothing else. 

    <example input>
    Information Consumption
    - Interested in world news
    - Prefers concise summaries of information
    - Values exhaustive, code-based explanations and examples
    </example input>
    <example output>
    📰
    </example output>
    "###;
    pub const FORMATTING_MEMORY: &'static str = r###"You are given a large collection of descriptions of user preferences, behaviors, traits, etc. You will help a personal AI assist the user by parsing out any redundancies present in the description. You will group the user description into distinct categories, and output each category inside a <memory></memory> tag. 

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
    Technical Expertise
    - Advanced Rust programmer, experienced in web development and AI systems
    - Proficient in Swift and SwiftUI, specializing in macOS and iOS development
    - Knowledgeable in git and software development processes
    - Uses Neovim and various Rust libraries
    - Demonstrates understanding of database interactions and API routes in Rust
    </memory>
    
    <memory>
    Current Projects
    - Developing a memory generation system for an AI application
    - Creating a SwiftUI/macOS application with custom window management
    - Implementing a complex chat application with branching conversations
    - Working on image and text evaluation for different language models
    - Developing a Rust project involving database operations and API routes
    - Adding a new field (`model_id`) to a `Message` struct in a Rust project
    </memory>
    
    <memory>
    Interests and Technical Focus
    - Low-level window behavior and event handling in macOS
    - UI/UX development, particularly interactive components like hover states
    - Exploring capabilities of AI models, specifically Claude Sonnet 3.5 and GPT-4 with vision
    - Custom window management in macOS applications
    </memory>
    
    <memory>
    Communication and Interaction Style
    - Prefers brief, concise responses and may not engage in lengthy conversations
    - Communicates casually and directly, often using abbreviated language
    - Frequently inputs random strings of characters or short, meaningless messages
    - Tends to start conversations with casual greetings like "hello" or "what's up"
    - Does not typically respond to requests for clarification
    - Demonstrates persistence in interaction, continuing to send messages despite receiving explanations about unclear inputs
    - May use British/Australian slang
    </memory>
    
    <memory>
    Problem-Solving and Learning Preferences
    - Values detailed, technical explanations and robust error handling in code
    - Prefers structured, step-by-step explanations for debugging or information
    - Detail-oriented and seeks rigorous, well-thought-out solutions
    - Expects code-focused explanations with specific examples and action items
    </memory>
    
    <memory>
    Specific Technical Challenges
    - Encountering errors related to the format of image data in AI model projects
    - Facing compilation errors due to type mismatches and missing fields after adding new fields to Rust structs
    - Seeking advice on specific UI/UX implementation challenges in SwiftUI
    </memory>
    
    <memory>
    Behavioral Patterns
    - Appears to be testing or exploring AI system capabilities through various inputs
    - Maintains a consistent behavior pattern throughout conversations
    - Shows interest in technical topics, particularly related to AI models and programming
    </memory>
    </output> 
    
    "###;
}
