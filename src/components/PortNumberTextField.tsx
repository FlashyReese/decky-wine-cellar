import React, { ChangeEvent, useCallback } from "react";
import { TextField, TextFieldProps } from "decky-frontend-lib";

interface PortNumberTextFieldProps extends TextFieldProps {
  startingPortNumber?: number;
  endingPortNumber?: number;
  onPortNumberChange: (portNumber: number) => void;
}

const PortNumberTextField: React.FC<PortNumberTextFieldProps> = ({
  startingPortNumber,
  endingPortNumber,
  onPortNumberChange,
  ...restProps
}) => {
  const starting = startingPortNumber != null ? startingPortNumber : 0;
  const ending = endingPortNumber != null ? endingPortNumber : 65535;
  if (starting > ending) {
    throw new Error(
      "Starting port number cannot be greater than ending port number",
    );
  }
  const handleChange = useCallback(
    (event: ChangeEvent<HTMLInputElement>) => {
      const input = event.target.value;
      const portNumber = parseInt(input, 10);

      // Check if the input is a valid integer and falls within a valid port range (0 - 65535)
      if (
        !isNaN(portNumber) &&
        portNumber >= starting &&
        portNumber <= ending
      ) {
        onPortNumberChange(portNumber);
      }
    },
    [onPortNumberChange],
  );

  return (
    <TextField
      {...restProps}
      onChange={handleChange}
      rangeMin={1}
      mustBeNumeric={true}
      style={{ minWidth: "80px" }}
    />
  );
};

export default PortNumberTextField;
