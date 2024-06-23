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
}
