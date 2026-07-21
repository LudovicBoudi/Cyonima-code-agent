import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

// Thème violet unique (pas de thème clair). La classe `dark` garde les
// variantes `dark:` de Tailwind actives ; les couleurs viennent de :root.
document.documentElement.classList.add("dark");

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
