import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { WhisProvider } from "./shared/state/WhisContext";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <WhisProvider>
      <App />
    </WhisProvider>
  </React.StrictMode>,
);
