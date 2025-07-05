# 🧠 Multi-Client GUI Application (ChatGui & WebGui)

This application provides a graphical interface for simulating communication between multiple clients and servers, using Bevy and Egui. It supports two separate GUI modes: `ChatGui` and `WebGui`, each enabling unique interactions between clients and servers via `crossbeam` channels.

---

## 📦 Architecture Overview

- **Crossbeam Channels** are used for communication between the GUI and backend clients.
- Each client can be interacted with individually, enabling multi-client simulations.
- Two distinct GUIs:
  - `ChatGui` — focused on real-time messaging.
  - `WebGui` — focused on content/media retrieval.

---

## 💬 ChatGui

### 🎛 Layout and Features

- **Left Panel:**
  - Displays a list of available clients.
  - Clicking a client:
    - Selects it.
    - Sends a `ServerType` request to its connected topology.
    - Shows the `ViewState` for the client.
  
- **Right Panel:**
  - Displays available servers of type `ChatServer` after the `ServerType` response is received.
  - Each server has a button:
    - Initially shows "Register".
    - Clicking it sends a `register_to` request to the server.
    - On successful response, button updates to show the server ID.
    - Clicking the ID button selects the server and requests the list of available clients for chatting.

- **Chat Targets:**
  - New buttons below the client list:
    - Labelled "Chat with N.Client", one for each client registered to the selected server (excluding the current client).
    - Allow sending messages between clients.

- **Bottom Bar:**
  - Text input bar for sending messages.
  - Attachment button to open a window with available media (images, audio).
  
- **Chat View:**
  - Sent messages: shown on the right.
  - Received messages: shown on the left.

---

## 🌐 WebGui

### 🎛 Layout and Features

- **Left Panel:**
  - Displays client buttons to select which client is active.

- **Right Panel:**
  - Displays server buttons:
    - `TextServer` and `MediaServer`.
    - Selecting a server allows interaction.

- **Request Buttons:**
  - `GetAllMedia` and `GetAllText` buttons for the selected server.
  - Send a request to retrieve all media or text content.

- **Center Panel:**
  - Displays the responses:
    - **Text responses**: shown on the left.
    - **Media responses**: shown on the right.

---


