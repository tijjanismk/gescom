// equipe/main.tsx — l'entrée de Gescom Équipe (PLAN-EQUIPE, D28-D29).
import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/archivo";
import "../index.css";
import { installerRemonteeErreurs } from "@/lib/pont";
import { AppEquipe } from "./AppEquipe";

installerRemonteeErreurs();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppEquipe />
  </React.StrictMode>,
);
