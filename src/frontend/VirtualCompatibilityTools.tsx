import {
  DialogBody,
  DialogButton,
  DialogControlsSection,
  DialogControlsSectionHeader,
  Field,
  TextField,
} from "decky-frontend-lib";
import { useState, useEffect } from "react";
import { FaBox } from "react-icons/fa";

export default function VirtualCompatibilityTools() {
  const [textFieldValue, setTextFieldValue] = useState("");
  const [isTextExists, setIsTextExists] = useState(false);

  const handleTextFieldChange = (event: { target: { value: any } }) => {
    const inputValue = event.target.value;
    const filteredValue = inputValue.replace(/[^a-zA-Z0-9 ]/g, "");
    setTextFieldValue(filteredValue);
  };

  const isTextFieldEmpty = textFieldValue.trim() === "";

  useEffect(() => {
    // Simulated WebSocket implementation
    const checkTextExists = async () => {
      // Replace this with your actual WebSocket logic
      // For example, you can use the WebSocket API or a WebSocket library

      // Simulate an asynchronous request to check if the text exists
      const response = await fetch(
        `your_websocket_endpoint?text=${textFieldValue}`,
      );
      const data = await response.json();

      // Set the value of isTextExists based on the response from the server
      setIsTextExists(data.exists);
    };

    if (!isTextFieldEmpty) {
      checkTextExists().then(() => {});
    }
  }, [textFieldValue]);

  return (
    <DialogBody>
      <DialogControlsSection>
        <DialogControlsSectionHeader>
          Create Virtual Compatibility Tool
        </DialogControlsSectionHeader>
        <Field
          label="Name"
          indentLevel={1}
          description={
            <TextField
              label="Name"
              value={textFieldValue}
              description={
                isTextExists
                  ? "There is already a virtual compatibility tool with this name"
                  : "Enter a name for your virtual compatibility tool"
              }
              onChange={handleTextFieldChange}
            />
          }
          icon={<FaBox style={{ display: "block" }} />}
        />
        <Field
          description={
            <DialogButton disabled={isTextFieldEmpty || isTextExists}>
              Test Button
            </DialogButton>
          }
        />
      </DialogControlsSection>
    </DialogBody>
  );
}
