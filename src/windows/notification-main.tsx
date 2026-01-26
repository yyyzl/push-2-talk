import React from "react";
import ReactDOM from "react-dom/client";
import NotificationWindow from "./NotificationWindow";
import "../index.css";

ReactDOM.createRoot(document.getElementById("notification-root")!).render(
  <React.StrictMode>
    <NotificationWindow />
  </React.StrictMode>
);
