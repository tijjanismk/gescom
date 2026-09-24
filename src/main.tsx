import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "@fontsource-variable/archivo";
import './index.css';
import { installerRemonteeErreurs } from "@/lib/pont";

// Avant le premier rendu : une erreur au démarrage compte aussi (B-2).
installerRemonteeErreurs();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
