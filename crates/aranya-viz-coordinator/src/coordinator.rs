use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use axum::{
    response::Html,
    routing::get,
    Router,
};
use tokio::sync::{broadcast, RwLock};
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::info;

use crate::{
    daemon_manager::DaemonManager,
    websocket::websocket_handler,
    AppState, AppStateInner, WsMessage,
};

/// Main coordinator service
pub struct Coordinator {
    bind_addr: SocketAddr,
    static_dir: Option<std::path::PathBuf>,
}

impl Coordinator {
    pub fn new(bind_addr: SocketAddr, static_dir: Option<std::path::PathBuf>) -> Self {
        Self { bind_addr, static_dir }
    }

    pub async fn run(self) -> Result<()> {
        // Initialize shared state
        let state: AppState = Arc::new(RwLock::new(AppStateInner::new()));
        
        // Initialize daemon manager
        let daemon_manager = Arc::new(DaemonManager::new(state.clone()));
        
        // Create broadcast channel for WebSocket messages
        let (tx, _rx) = broadcast::channel::<WsMessage>(1000);

        // Build router
        let mut router = Router::new()
            .route("/ws", get(websocket_handler))
            .layer(TraceLayer::new_for_http())
            .layer(CorsLayer::permissive())
            .with_state((state, daemon_manager, tx));

        // Add static file serving if directory is provided
        if let Some(static_dir) = &self.static_dir {
            info!("Serving static files from: {}", static_dir.display());
            router = router.nest_service("/", ServeDir::new(static_dir));
        } else {
            // Serve basic HTML if no static directory provided
            router = router.route("/", get(serve_basic_html));
        }

        info!("Starting Aranya Visualization Coordinator on {}", self.bind_addr);

        let listener = tokio::net::TcpListener::bind(self.bind_addr).await?;
        axum::serve(listener, router).await?;

        Ok(())
    }
}

async fn serve_basic_html() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html>
<head>
    <title>Aranya Visualization</title>
    <style>
        :root {
            --primary-color: #2563eb;
            --secondary-color: #64748b;
            --accent-color: #0ea5e9;
            --success-color: #22c55e;
            --warning-color: #f59e0b;
            --error-color: #ef4444;
            --background-color: #f8fafc;
            --surface-color: #ffffff;
            --text-primary: #1e293b;
            --text-secondary: #64748b;
            --border-color: #e2e8f0;
            --shadow-sm: 0 1px 2px 0 rgb(0 0 0 / 0.05);
            --shadow-md: 0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1);
            --shadow-lg: 0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1);
            --radius-sm: 0.375rem;
            --radius-md: 0.5rem;
            --radius-lg: 0.75rem;
        }
        
        * {
            box-sizing: border-box;
        }
        
        body { 
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0; 
            padding: 0; 
            background: var(--background-color);
            color: var(--text-primary);
            line-height: 1.6;
        }
        
        .container { 
            max-width: 100vw; 
            margin: 0; 
            padding: 1rem;
            display: grid;
            grid-template-columns: 1fr;
            gap: 1rem;
            height: 100vh;
        }
        
        .header { 
            text-align: center; 
            padding: 1rem;
            background: var(--surface-color);
            border-radius: var(--radius-lg);
            box-shadow: var(--shadow-sm);
            border: 1px solid var(--border-color);
        }
        
        .header h1 {
            margin: 0 0 0.5rem 0;
            color: var(--primary-color);
            font-size: 1.875rem;
            font-weight: 700;
        }
        
        .header p {
            margin: 0;
            color: var(--text-secondary);
            font-size: 0.875rem;
        }
        
        .main-content {
            display: grid;
            grid-template-columns: 320px 1fr 320px;
            gap: 1rem;
            flex: 1;
            min-height: 0;
        }
        
        .canvas-container { 
            background: var(--surface-color);
            border-radius: var(--radius-lg);
            overflow: hidden;
            box-shadow: var(--shadow-md);
            border: 1px solid var(--border-color);
            position: relative;
            display: flex;
            flex-direction: column;
        }
        
        .canvas-header {
            padding: 1rem;
            background: var(--surface-color);
            border-bottom: 1px solid var(--border-color);
            display: flex;
            justify-content: space-between;
            align-items: center;
            flex-wrap: wrap;
            gap: 0.5rem;
        }
        
        .canvas-viewport {
            flex: 1;
            position: relative;
            overflow: hidden;
            background: var(--texture-bg, #f8fafc);
            background-image: var(--texture-pattern, 
                linear-gradient(rgba(0,0,0,.03) 1px, transparent 1px),
                linear-gradient(90deg, rgba(0,0,0,.03) 1px, transparent 1px));
            background-size: var(--texture-size, 40px 40px);
        }
        
        #canvas { 
            display: block; 
            cursor: crosshair; 
            background: transparent;
            width: 100%;
            height: 100%;
        }
        
        .controls { 
            display: flex;
            gap: 0.5rem;
            align-items: center;
            flex-wrap: wrap;
        }
        
        .controls button, .btn { 
            padding: 0.5rem 1rem;
            border: 1px solid var(--border-color);
            background: var(--surface-color);
            color: var(--text-primary);
            border-radius: var(--radius-md);
            cursor: pointer;
            font-size: 0.875rem;
            font-weight: 500;
            transition: all 0.2s ease;
            display: inline-flex;
            align-items: center;
            gap: 0.5rem;
        }
        
        .controls button:hover, .btn:hover {
            background: var(--background-color);
            border-color: var(--primary-color);
            transform: translateY(-1px);
        }
        
        .controls button.active, .btn.active {
            background: var(--primary-color);
            color: white;
            border-color: var(--primary-color);
        }
        
        .status { 
            background: var(--surface-color);
            padding: 0.75rem 1rem;
            border-radius: var(--radius-md);
            box-shadow: var(--shadow-sm);
            border: 1px solid var(--border-color);
            font-size: 0.875rem;
            color: var(--text-secondary);
        }
        
        .texture-selector {
            background: var(--surface-color);
            border-radius: var(--radius-lg);
            padding: 1rem;
            box-shadow: var(--shadow-sm);
            border: 1px solid var(--border-color);
            margin-bottom: 1rem;
        }
        
        .texture-selector h4 {
            margin: 0 0 1rem 0;
            color: var(--text-primary);
            font-size: 1rem;
            font-weight: 600;
        }
        
        .texture-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(60px, 1fr));
            gap: 0.5rem;
        }
        
        .texture-option {
            aspect-ratio: 1;
            border: 2px solid var(--border-color);
            border-radius: var(--radius-md);
            cursor: pointer;
            transition: all 0.2s ease;
            background-size: cover;
            background-position: center;
            position: relative;
            overflow: hidden;
        }
        
        .texture-option:hover {
            border-color: var(--primary-color);
            transform: scale(1.05);
        }
        
        .texture-option.selected {
            border-color: var(--primary-color);
            box-shadow: 0 0 0 2px var(--primary-color);
        }
        
        .texture-option::after {
            content: '';
            position: absolute;
            inset: 0;
            background: var(--texture-bg, #f8fafc);
            background-image: var(--texture-pattern);
            background-size: var(--texture-size, 20px 20px);
        }
        
        .texture-option.selected::after {
            content: '✓';
            position: absolute;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            color: white;
            font-size: 18px;
            font-weight: bold;
            text-shadow: 0 0 3px rgba(0,0,0,0.8);
            background: none;
            z-index: 1;
        }
        
        .texture-option.custom {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            font-size: 24px;
            font-weight: bold;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        
        .texture-option.custom::after {
            display: none;
        }
        
        .texture-option.custom:hover {
            background: linear-gradient(135deg, #5a67d8 0%, #6b46c1 100%);
        }
        
        .custom-texture-panel {
            margin-top: 1rem;
            padding: 1rem;
            background: var(--surface-color);
            border: 1px solid var(--border-color);
            border-radius: var(--radius-md);
            box-shadow: var(--shadow-sm);
            overflow-x: hidden;
            width: 100%;
            box-sizing: border-box;
        }
        
        .custom-texture-panel h5 {
            margin: 0 0 0.75rem 0;
            color: var(--text-primary);
            font-size: 0.875rem;
            font-weight: 600;
        }
        
        .file-input {
            width: 100%;
            padding: 0.5rem;
            border: 1px solid var(--border-color);
            border-radius: var(--radius-sm);
            background: var(--background-color);
            font-size: 0.875rem;
            color: var(--text-primary);
        }
        
        .opacity-slider {
            width: 100%;
            margin: 0.25rem 0;
        }
        
        .theme-select {
            width: 100%;
            padding: 0.5rem;
            border: 1px solid var(--border-color);
            border-radius: var(--radius-sm);
            background: var(--background-color);
            font-size: 0.875rem;
            color: var(--text-primary);
            cursor: pointer;
        }
        
        .theme-select:focus {
            outline: none;
            border-color: var(--primary-color);
            box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.1);
        }
        
        .icon-input {
            width: 100%;
            padding: 0.5rem;
            border: 1px solid var(--border-color);
            border-radius: var(--radius-sm);
            background: var(--background-color);
            font-size: 1.25rem;
            text-align: center;
            color: var(--text-primary);
        }
        
        .individual-icons {
            margin-top: 0.75rem;
            padding: 0.5rem;
            background: var(--background-color);
            border-radius: var(--radius-sm);
            border: 1px solid var(--border-color);
            max-height: 300px;
            overflow-y: auto;
            overflow-x: hidden;
        }
        
        .node-icon-item {
            display: flex;
            flex-direction: column;
            padding: 0.75rem 0.5rem;
            margin: 0.5rem 0;
            background: var(--surface-color);
            border-radius: var(--radius-sm);
            border: 1px solid var(--border-color);
            gap: 0.5rem;
        }
        
        .node-icon-header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            width: 100%;
            min-width: 0;
        }
        
        .node-icon-item .node-id {
            font-size: 0.75rem;
            color: var(--text-secondary);
            font-family: monospace;
            font-weight: 600;
            flex: 1;
            min-width: 0;
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
        }
        
        .node-icon-controls {
            display: flex;
            align-items: center;
            gap: 0.375rem;
            width: 100%;
            flex-wrap: wrap;
        }
        
        .node-icon-controls-row {
            display: flex;
            align-items: center;
            gap: 0.375rem;
            flex: 1;
            min-width: 0;
        }
        
        .icon-preview {
            width: 36px;
            height: 36px;
            border: 1px solid var(--border-color);
            border-radius: var(--radius-sm);
            display: flex;
            align-items: center;
            justify-content: center;
            background: var(--background-color);
            font-size: 1.25rem;
            overflow: hidden;
            flex-shrink: 0;
        }
        
        .icon-preview img {
            width: 100%;
            height: 100%;
            object-fit: cover;
            border-radius: var(--radius-sm);
        }
        
        .icon-input {
            flex: 1;
            min-width: 50px;
            padding: 0.25rem 0.5rem;
            border: 1px solid var(--border-color);
            border-radius: var(--radius-sm);
            background: var(--background-color);
            font-size: 1rem;
            text-align: center;
            color: var(--text-primary);
        }
        
        .icon-upload-btn {
            padding: 0.375rem 0.5rem;
            font-size: 0.7rem;
            background: var(--accent-color);
            color: white;
            border: none;
            border-radius: var(--radius-sm);
            cursor: pointer;
            transition: background-color 0.2s ease;
            white-space: nowrap;
            font-weight: 600;
        }
        
        .icon-upload-btn:hover {
            background: var(--primary-color);
        }
        
        .icon-clear-btn {
            padding: 0.375rem 0.5rem;
            font-size: 0.875rem;
            background: var(--error-color);
            color: white;
            border: none;
            border-radius: var(--radius-sm);
            cursor: pointer;
            transition: background-color 0.2s ease;
            width: 28px;
            height: 28px;
            display: flex;
            align-items: center;
            justify-content: center;
            flex-shrink: 0;
        }
        
        .icon-clear-btn:hover {
            background: #dc2626;
        }
        
        .hidden-file-input {
            display: none;
        }
        
        .node-icons-header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            margin-bottom: 0.75rem;
        }
        
        .toggle-btn {
            background: none;
            border: 1px solid var(--border-color);
            border-radius: var(--radius-sm);
            padding: 0.25rem 0.5rem;
            color: var(--text-secondary);
            cursor: pointer;
            transition: all 0.2s ease;
            font-size: 0.75rem;
            min-width: 24px;
            height: 24px;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        
        .toggle-btn:hover {
            background: var(--background-color);
            border-color: var(--primary-color);
            color: var(--primary-color);
        }
        
        .node-icon-section.collapsed {
            display: none;
        }
        
        .custom-texture-actions {
            margin-top: 1rem;
            display: flex;
            gap: 0.5rem;
        }
        
        .custom-texture-actions .btn {
            flex: 1;
            padding: 0.5rem;
            font-size: 0.875rem;
        }
        .panel {
            background: var(--surface-color);
            border-radius: var(--radius-lg);
            padding: 1rem;
            box-shadow: var(--shadow-sm);
            border: 1px solid var(--border-color);
            height: fit-content;
            max-height: calc(100vh - 8rem);
            overflow-y: auto;
            overflow-x: hidden;
            min-width: 0;
        }
        
        .panel h3 {
            margin: 0 0 1rem 0;
            color: var(--text-primary);
            font-size: 1.125rem;
            font-weight: 600;
            border-bottom: 1px solid var(--border-color);
            padding-bottom: 0.5rem;
        }
        
        .panel h4 {
            margin: 1.5rem 0 0.75rem 0;
            color: var(--text-primary);
            font-size: 1rem;
            font-weight: 600;
        }
        
        .panel h4:first-child {
            margin-top: 0;
        }
        
        .form-group {
            margin-bottom: 1rem;
        }
        
        .form-group label {
            display: block;
            margin-bottom: 0.5rem;
            font-weight: 500;
            color: var(--text-primary);
            font-size: 0.875rem;
        }
        
        .form-group input, .form-group select, .form-group textarea {
            width: 100%;
            padding: 0.75rem;
            border: 1px solid var(--border-color);
            border-radius: var(--radius-md);
            font-size: 0.875rem;
            transition: border-color 0.2s ease;
            background: var(--surface-color);
            color: var(--text-primary);
        }
        
        .form-group input:focus, .form-group select:focus, .form-group textarea:focus {
            outline: none;
            border-color: var(--primary-color);
            box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.1);
        }
        
        .form-group textarea {
            resize: vertical;
            min-height: 80px;
        }
        
        .btn-grid {
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 0.5rem;
            margin-bottom: 1rem;
        }
        
        .btn-full {
            grid-column: 1 / -1;
        }
        
        .btn-primary {
            background: var(--primary-color);
            color: white;
            border-color: var(--primary-color);
        }
        
        .btn-primary:hover {
            background: #1d4ed8;
            border-color: #1d4ed8;
        }
        
        .btn-success {
            background: var(--success-color);
            color: white;
            border-color: var(--success-color);
        }
        
        .btn-success:hover {
            background: #16a34a;
            border-color: #16a34a;
        }
        
        .btn-warning {
            background: var(--warning-color);
            color: white;
            border-color: var(--warning-color);
        }
        
        .btn-warning:hover {
            background: #d97706;
            border-color: #d97706;
        }
        
        .btn-purple {
            background: #9333ea;
            color: white;
            border-color: #9333ea;
        }
        
        .btn-purple:hover {
            background: #7c2d12;
            border-color: #7c2d12;
        }
        .context-menu {
            position: absolute;
            background: var(--surface-color);
            border: 1px solid var(--border-color);
            border-radius: var(--radius-md);
            padding: 0.5rem 0;
            display: none;
            z-index: 1000;
            box-shadow: var(--shadow-lg);
            min-width: 180px;
        }
        
        .context-menu-item {
            padding: 0.75rem 1rem;
            cursor: pointer;
            font-size: 0.875rem;
            color: var(--text-primary);
            transition: background-color 0.2s ease;
        }
        
        .context-menu-item:hover {
            background: var(--background-color);
        }
        .message-bubble {
            position: absolute;
            background: var(--primary-color);
            color: white;
            padding: 0.75rem 1rem;
            border-radius: 1rem;
            font-size: 0.8125rem;
            font-weight: 500;
            max-width: 280px;
            word-wrap: break-word;
            pointer-events: none;
            opacity: 0;
            transform: translateY(-10px) scale(0.8);
            transition: all 0.4s cubic-bezier(0.175, 0.885, 0.32, 1.275);
            z-index: 1001;
            box-shadow: var(--shadow-lg);
            border: 1px solid rgba(255,255,255,0.2);
        }
        
        .message-bubble.show {
            opacity: 1;
            transform: translateY(-50px) scale(1);
        }
        
        .message-bubble::after {
            content: '';
            position: absolute;
            bottom: -6px;
            left: 50%;
            transform: translateX(-50%);
            width: 0;
            height: 0;
            border-left: 6px solid transparent;
            border-right: 6px solid transparent;
            border-top: 6px solid var(--primary-color);
        }
        .team-item {
            padding: 0.75rem;
            margin: 0.5rem 0;
            background: var(--background-color);
            border-radius: var(--radius-md);
            cursor: pointer;
            border: 1px solid var(--border-color);
            transition: all 0.2s ease;
        }
        
        .team-item:hover {
            border-color: var(--primary-color);
            background: #eff6ff;
        }
        
        .team-item.selected {
            background: #eff6ff;
            border-color: var(--primary-color);
            box-shadow: 0 0 0 1px var(--primary-color);
        }
        
        .team-members {
            font-size: 0.75rem;
            color: var(--text-secondary);
            margin-top: 0.5rem;
        }
        
        .no-team-selected {
            color: var(--text-secondary);
            font-style: italic;
            font-size: 0.875rem;
        }
        
        .message-history {
            max-height: 200px;
            overflow-y: auto;
            border: 1px solid var(--border-color);
            border-radius: var(--radius-md);
            padding: 0.75rem;
            background: var(--background-color);
        }
        
        .message-item {
            margin-bottom: 0.75rem;
            padding: 0.5rem;
            background: var(--surface-color);
            border-radius: var(--radius-sm);
            border: 1px solid var(--border-color);
        }
        
        .message-item:last-child {
            margin-bottom: 0;
        }
        
        .message-author {
            font-weight: 600;
            color: var(--primary-color);
            font-size: 0.875rem;
        }
        
        .message-time {
            color: var(--text-secondary);
            font-size: 0.75rem;
        }
        
        .message-text {
            color: var(--text-primary);
            margin-top: 0.25rem;
            font-size: 0.875rem;
        }
        
        .role-legend {
            font-size: 0.75rem;
            color: var(--text-secondary);
            margin-top: 0.75rem;
            padding: 0.75rem;
            background: var(--background-color);
            border-radius: var(--radius-md);
            border: 1px solid var(--border-color);
        }
        
        .role-legend strong {
            color: var(--text-primary);
            font-size: 0.8125rem;
        }
        
        .role-color {
            font-weight: 600;
        }
        
        @media (max-width: 1200px) {
            .main-content {
                grid-template-columns: 280px 1fr 280px;
            }
        }
        
        @media (max-width: 1024px) {
            .main-content {
                grid-template-columns: 1fr;
                grid-template-rows: auto 1fr auto;
            }
            
            .panel {
                max-height: 300px;
            }
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>Aranya Visualization</h1>
            <p>Click to place nodes. Drag to move them. Pan by dragging empty space. Zoom with mouse wheel. Right-click for options.</p>
        </div>
        
        <div class="main-content">
            <div class="panel">
                <div class="texture-selector">
                    <h4>🎨 Background Themes</h4>
                    <div class="texture-grid">
                        <div class="texture-option selected" data-theme="default" title="Default Grid"></div>
                        <div class="texture-option" data-theme="dark" title="Dark Theme"></div>
                        <div class="texture-option" data-theme="blueprint" title="Blueprint"></div>
                        <div class="texture-option" data-theme="organic" title="Organic"></div>
                        <div class="texture-option custom" data-theme="custom" title="Custom Texture Pack">+</div>
                    </div>
                    
                    <div id="customTexturePanel" class="custom-texture-panel" style="display: none;">
                        <h5>📸 Custom Background</h5>
                        <div class="form-group">
                            <label for="backgroundTheme">Interface Theme:</label>
                            <select id="backgroundTheme" class="theme-select">
                                <option value="light">☀️ Light Theme</option>
                                <option value="dark">🌙 Dark Theme</option>
                            </select>
                        </div>
                        <div class="form-group">
                            <label for="backgroundUpload">Upload Background Image:</label>
                            <input type="file" id="backgroundUpload" accept="image/*" class="file-input">
                        </div>
                        <div class="form-group">
                            <label for="backgroundOpacity">Background Opacity:</label>
                            <input type="range" id="backgroundOpacity" min="0" max="100" value="80" class="opacity-slider">
                            <span id="opacityValue">80%</span>
                        </div>
                        
                        <div class="node-icons-header">
                            <h5>🎯 Node Icons</h5>
                            <button id="toggleNodeIcons" class="toggle-btn">▼</button>
                        </div>
                        <div id="nodeIconSection" class="node-icon-section">
                            <div class="form-group">
                                <label>Default Icon for New Nodes:</label>
                                <input type="text" id="defaultNodeIcon" value="🔵" placeholder="🔵" class="icon-input">
                            </div>
                            <div id="individualNodeIcons" class="individual-icons"></div>
                        </div>
                        
                        <div class="custom-texture-actions">
                            <button id="saveCustomTexture" class="btn btn-primary">Save Custom Pack</button>
                            <button id="resetCustomTexture" class="btn btn-secondary">Reset</button>
                        </div>
                    </div>
                </div>
                
                <h3>📤 Send Message</h3>
                <div class="form-group">
                    <label for="messageSenderSelector">Sender Node:</label>
                    <select id="messageSenderSelector">
                        <option value="">-- Select Sender Node --</option>
                    </select>
                </div>
                <div class="form-group">
                    <label for="messageTeamSelector">Team:</label>
                    <select id="messageTeamSelector">
                        <option value="">-- Select Team --</option>
                    </select>
                </div>
                <div class="form-group">
                    <label for="messageInput">Message:</label>
                    <textarea id="messageInput" placeholder="Type your message here..."></textarea>
                </div>
                <button class="btn btn-primary btn-full" onclick="sendBroadcastMessage()" id="sendMessageBtn" disabled>
                    Send Message
                </button>
                
                <h4>📜 Recent Messages</h4>
                <div id="recentMessages" class="message-history">
                    <p style="color: var(--text-secondary); font-style: italic; text-align: center; margin: 1rem 0;">No messages yet</p>
                </div>
            </div>
            
            <div class="canvas-container">
                <div class="canvas-header">
                    <div class="controls">
                        <button onclick="selectTool('select')" id="selectTool" class="btn active">✋ Select/Move</button>
                        <button onclick="selectTool('place')" id="placeTool" class="btn">➕ Place Node</button>
                        <button onclick="selectTool('connect')" id="connectTool" class="btn">🔗 Connect</button>
                        <input type="text" id="nodeNameInput" placeholder="Node name..." value="" style="max-width: 150px;">
                    </div>
                    <div class="controls">
                        <button onclick="clearAll()" class="btn">🗑️ Clear All</button>
                        <button onclick="toggleConnections()" class="btn">👁️ Connections</button>
                        <button onclick="resetView()" class="btn">🔄 Reset View</button>
                        <span style="color: var(--text-secondary); font-size: 0.875rem;">Zoom: <span id="zoomLevel">100%</span></span>
                    </div>
                </div>
                <div class="canvas-viewport">
                    <canvas id="canvas"></canvas>
                </div>
            </div>
            
            <div class="panel">
                <h3>🏢 Team Management</h3>
                
                <h4>Create Team</h4>
                <div class="form-group">
                    <label for="teamNameInput">Team Name:</label>
                    <input type="text" id="teamNameInput" placeholder="Enter team name...">
                </div>
                <div class="btn-grid">
                    <select id="nodeSelector" class="form-group">
                        <option value="">-- Select Node --</option>
                    </select>
                    <button onclick="createTeamFromUI()" class="btn btn-primary">Create Team</button>
                </div>
                
                <h4>Join Team</h4>
                <div class="btn-grid">
                    <select id="joinNodeSelector" class="form-group">
                        <option value="">-- Select Node --</option>
                    </select>
                    <button onclick="joinTeamFromUI()" class="btn btn-primary">Join Team</button>
                </div>
                <div class="form-group">
                    <select id="availableTeamsSelector">
                        <option value="">-- Select Team to Join --</option>
                    </select>
                </div>
                
                <h4>Team Operations</h4>
                <div class="form-group">
                    <label for="teamSelector">Active Team:</label>
                    <select id="teamSelector" onchange="selectTeam()">
                        <option value="">-- Select Team for Operations --</option>
                    </select>
                </div>
                <p class="no-team-selected" id="selectedTeamInfo">No team selected for sync operations</p>
                
                <div class="btn-grid">
                    <button onclick="addAllNodesToSelectedTeam()" class="btn btn-success btn-full">
                        Add All Nodes to Team
                    </button>
                    <button onclick="autoSyncTeamMembers()" class="btn btn-primary">
                        Auto-Sync All
                    </button>
                    <button onclick="syncAllMembersFromOwner()" class="btn btn-purple">
                        Owner Sync
                    </button>
                    <button onclick="quickTeamSetup()" class="btn btn-warning btn-full">
                        Quick Team Setup
                    </button>
                </div>
                
                <div class="role-legend">
                    <strong>Role Colors:</strong><br>
                    • <span class="role-color" style="color: #FF5722;">Red</span>: Owner<br>
                    • <span class="role-color" style="color: #FF9800;">Orange</span>: Admin<br>
                    • <span class="role-color" style="color: #9C27B0;">Purple</span>: Operator<br>
                    • <span class="role-color" style="color: #2196F3;">Blue</span>: Member<br>
                    <br>
                    <strong>Sync Types:</strong><br>
                    • <span style="color: var(--primary-color);">Auto-Sync</span>: Full mesh<br>
                    • <span style="color: #9C27B0;">Owner Sync</span>: Owner → Members<br>
                    <em>Arrows show data flow direction</em>
                </div>
                
                <h4>All Teams</h4>
                <div id="teamsList"></div>
            </div>
        </div>
        
        <div class="status" id="status">
            Connecting to WebSocket...
        </div>
    </div>

    <div class="context-menu" id="contextMenu">
        <div class="context-menu-item" onclick="deleteSelectedNode()">Delete Node</div>
        <div class="context-menu-item" onclick="createTeamForNode()">Create Team</div>
        <div class="context-menu-item" onclick="joinTeamDialog()">Join Team</div>
        <div class="context-menu-item" onclick="removeSyncPeers()">Remove Sync Peers</div>
        <div class="context-menu-item" onclick="sendMessageToNode()">Send Message</div>
        <div class="context-menu-item" onclick="viewNodeInfo()">View Info</div>
    </div>
    
    <div class="message-panel">
        <h3>Send Message</h3>
        <div style="margin-bottom: 10px;">
            <label for="messageSenderSelector">Select Sender Node:</label>
            <select id="messageSenderSelector" style="width: 100%; padding: 5px; margin-top: 5px;">
                <option value="">-- Select Sender Node --</option>
            </select>
        </div>
        <div style="margin-bottom: 10px;">
            <label for="messageTeamSelector">Select Team:</label>
            <select id="messageTeamSelector" style="width: 100%; padding: 5px; margin-top: 5px;">
                <option value="">-- Select Team to Send Message --</option>
            </select>
        </div>
        <div style="margin-bottom: 10px;">
            <label for="messageInput">Message:</label>
            <textarea id="messageInput" class="message-input" placeholder="Type your message here..."></textarea>
        </div>
        <button class="message-send-btn" onclick="sendBroadcastMessage()" id="sendMessageBtn" disabled>Send Message</button>
        
        <div style="margin-top: 20px; border-top: 1px solid #eee; padding-top: 15px;">
            <h4>Recent Messages</h4>
            <div id="recentMessages" style="max-height: 200px; overflow-y: auto; font-size: 12px;">
                <p style="color: #999; font-style: italic;">No messages yet</p>
            </div>
        </div>
    </div>
    
    <div class="teams-panel">
        <h3>Team Management</h3>
        
        <div class="team-section">
            <h4>Create Team</h4>
            <div style="display: flex; gap: 10px; margin-bottom: 10px;">
                <select id="nodeSelector" style="flex: 1; padding: 5px;">
                    <option value="">-- Select Node --</option>
                </select>
                <button onclick="createTeamFromUI()" style="padding: 5px 10px;">Create Team</button>
            </div>
            <input type="text" id="teamNameInput" placeholder="Team name..." style="width: 100%; padding: 5px; margin-bottom: 10px;">
        </div>
        
        <div class="team-section">
            <h4>Join Team</h4>
            <div style="display: flex; gap: 10px; margin-bottom: 10px;">
                <select id="joinNodeSelector" style="flex: 1; padding: 5px;">
                    <option value="">-- Select Node --</option>
                </select>
                <button onclick="joinTeamFromUI()" style="padding: 5px 10px;">Join Team</button>
            </div>
            <select id="availableTeamsSelector" style="width: 100%; padding: 5px; margin-bottom: 10px;">
                <option value="">-- Select Team to Join --</option>
            </select>
        </div>
        
        <div class="team-section">
            <h4>Sync Operations</h4>
            <p class="no-team-selected" id="selectedTeamInfo">No team selected for sync operations</p>
            <select id="teamSelector" onchange="selectTeam()" style="width: 100%; padding: 5px; margin: 10px 0;">
                <option value="">-- Select Team for Sync --</option>
            </select>
        </div>
        
        <div class="team-section">
            <h4>Team Actions</h4>
            <div style="display: flex; flex-direction: column; gap: 5px; margin-bottom: 15px;">
                <button onclick="addAllNodesToSelectedTeam()" style="padding: 8px; background: #4CAF50; color: white; border: none; border-radius: 4px;">
                    Add All Nodes to Selected Team
                </button>
                <button onclick="autoSyncTeamMembers()" style="padding: 8px; background: #2196F3; color: white; border: none; border-radius: 4px;" title="Create bidirectional sync connections between all team members (full mesh)">
                    Auto-Sync All Team Members
                </button>
                <button onclick="syncAllMembersFromOwner()" style="padding: 8px; background: #9C27B0; color: white; border: none; border-radius: 4px;" title="Create sync connections from all team members to the owner. This ensures members receive owner commands for team management operations.">
                    Make Owner Sync Peer for All
                </button>
                <button onclick="quickTeamSetup()" style="padding: 8px; background: #FF9800; color: white; border: none; border-radius: 4px;">
                    Quick Team Setup (All Nodes)
                </button>
            </div>
            <div style="font-size: 11px; color: #666; margin-top: 10px; padding: 8px; background: #f9f9f9; border-radius: 4px;">
                <strong>Node Role Outlines:</strong><br>
                • <span style="color: #FF5722; font-weight: bold;">Red</span>: Owner<br>
                • <span style="color: #FF9800; font-weight: bold;">Orange</span>: Admin<br>
                • <span style="color: #9C27B0; font-weight: bold;">Purple</span>: Operator<br>
                • <span style="color: #2196F3; font-weight: bold;">Blue</span>: Member<br>
                <br>
                <strong>Sync Types:</strong><br>
                • <span style="color: #2196F3;">Auto-Sync All</span>: Full mesh (bidirectional)<br>
                • <span style="color: #9C27B0;">Owner Sync Peer</span>: Owner → Members (command flow)<br>
                <em>Arrows show data/command flow direction</em>
            </div>
        </div>
        
        <div class="team-section">
            <h4>All Teams</h4>
            <div id="teamsList"></div>
        </div>
    </div>

    <script>
        // Texture pack system
        const texturePacks = {
            default: {
                name: 'Default Grid',
                bg: '#f8fafc',
                pattern: 'linear-gradient(rgba(0,0,0,.03) 1px, transparent 1px), linear-gradient(90deg, rgba(0,0,0,.03) 1px, transparent 1px)',
                size: '40px 40px',
                nodeIcons: '🔵'
            },
            dark: {
                name: 'Dark Theme',
                bg: '#1a1a1a',
                pattern: 'linear-gradient(rgba(255,255,255,.1) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,.1) 1px, transparent 1px)',
                size: '40px 40px',
                nodeIcons: '⚫'
            },
            blueprint: {
                name: 'Blueprint',
                bg: '#1e3a8a',
                pattern: 'linear-gradient(rgba(255,255,255,.2) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,.2) 1px, transparent 1px)',
                size: '30px 30px',
                nodeIcons: '🔷'
            },
            organic: {
                name: 'Organic',
                bg: '#065f46',
                pattern: 'radial-gradient(circle at 20px 20px, rgba(255,255,255,.1) 1px, transparent 1px)',
                size: '40px 40px',
                nodeIcons: '🟢'
            },
            custom: {
                name: 'Custom Texture Pack',
                bg: '#f8fafc',
                pattern: null,
                backgroundImage: null,
                backgroundOpacity: 80,
                backgroundTheme: 'light', // 'light' or 'dark'
                size: '40px 40px',
                nodeIcons: '🔵',
                customNodeIcons: new Map(), // nodeId -> icon (text/emoji)
                customNodeImages: new Map() // nodeId -> image data URL
            }
        };
        
        // Custom texture pack management
        let customBackgroundImage = null;
        
        // Image cache for node icons
        const nodeImageCache = new Map(); // imageData -> HTMLImageElement
        
        let currentTexturePack = 'default';
        
        function initializeTexturePacks() {
            const textureOptions = document.querySelectorAll('.texture-option');
            
            textureOptions.forEach((option, index) => {
                const theme = option.dataset.theme;
                const pack = texturePacks[theme];
                
                if (pack && theme !== 'custom') {
                    option.style.setProperty('--texture-bg', pack.bg);
                    option.style.setProperty('--texture-pattern', pack.pattern);
                    option.style.setProperty('--texture-size', pack.size);
                }
                
                option.addEventListener('click', () => {
                    if (theme === 'custom') {
                        toggleCustomTexturePanel();
                    } else {
                        setTexturePack(theme);
                    }
                });
            });
            
            // Initialize custom texture pack controls
            initializeCustomTextureControls();
        }
        
        function toggleCustomTexturePanel() {
            const panel = document.getElementById('customTexturePanel');
            const isVisible = panel.style.display !== 'none';
            
            if (isVisible) {
                panel.style.display = 'none';
            } else {
                panel.style.display = 'block';
                updateIndividualNodeIcons();
                setTexturePack('custom');
            }
        }
        
        function initializeCustomTextureControls() {
            // Background theme selector
            const backgroundTheme = document.getElementById('backgroundTheme');
            backgroundTheme.addEventListener('change', (e) => {
                texturePacks.custom.backgroundTheme = e.target.value;
                if (currentTexturePack === 'custom') {
                    applyCustomTheme();
                }
            });
            
            // Background upload
            const backgroundUpload = document.getElementById('backgroundUpload');
            backgroundUpload.addEventListener('change', handleBackgroundUpload);
            
            // Opacity slider
            const opacitySlider = document.getElementById('backgroundOpacity');
            const opacityValue = document.getElementById('opacityValue');
            opacitySlider.addEventListener('input', (e) => {
                const opacity = parseInt(e.target.value);
                texturePacks.custom.backgroundOpacity = opacity;
                opacityValue.textContent = opacity + '%';
                if (currentTexturePack === 'custom') {
                    updateCustomBackground();
                }
            });
            
            // Default node icon
            const defaultNodeIcon = document.getElementById('defaultNodeIcon');
            defaultNodeIcon.addEventListener('input', (e) => {
                texturePacks.custom.nodeIcons = e.target.value || '🔵';
                if (currentTexturePack === 'custom') {
                    draw();
                }
            });
            
            // Save and reset buttons
            document.getElementById('saveCustomTexture').addEventListener('click', saveCustomTexturePack);
            document.getElementById('resetCustomTexture').addEventListener('click', resetCustomTexturePack);
            
            // Toggle button for node icons section
            document.getElementById('toggleNodeIcons').addEventListener('click', toggleNodeIconsSection);
        }
        
        function handleBackgroundUpload(event) {
            const file = event.target.files[0];
            if (file && file.type.startsWith('image/')) {
                const reader = new FileReader();
                reader.onload = (e) => {
                    customBackgroundImage = e.target.result;
                    texturePacks.custom.backgroundImage = customBackgroundImage;
                    if (currentTexturePack === 'custom') {
                        updateCustomBackground();
                    }
                };
                reader.readAsDataURL(file);
            }
        }
        
        function updateCustomBackground() {
            // For custom backgrounds with images, we'll draw them in the canvas
            // instead of using CSS, so we can apply viewport transformations
            const viewport = document.querySelector('.canvas-viewport');
            const pack = texturePacks.custom;
            
            if (pack.backgroundImage) {
                // Clear CSS background since we'll draw in canvas
                viewport.style.setProperty('--texture-bg', 'transparent');
                viewport.style.setProperty('--texture-pattern', 'none');
                viewport.style.opacity = 1;
                
                // Create background image element if not cached
                if (!customBackgroundImage || customBackgroundImage.src !== pack.backgroundImage) {
                    customBackgroundImage = new Image();
                    customBackgroundImage.src = pack.backgroundImage;
                    customBackgroundImage.onload = () => {
                        draw(); // Redraw when image loads
                    };
                }
            } else {
                // Use CSS background for non-image patterns
                viewport.style.setProperty('--texture-bg', pack.bg);
                viewport.style.setProperty('--texture-pattern', pack.pattern || 'none');
                viewport.style.setProperty('--texture-size', pack.size);
                viewport.style.opacity = 1;
                customBackgroundImage = null;
            }
            
            draw();
        }
        
        function updateIndividualNodeIcons() {
            const container = document.getElementById('individualNodeIcons');
            container.innerHTML = '';
            
            if (nodes.size === 0) {
                container.innerHTML = '<div style="color: var(--text-secondary); font-style: italic; font-size: 0.875rem;">No nodes available. Create some nodes first.</div>';
                return;
            }
            
            nodes.forEach((node, nodeId) => {
                const item = document.createElement('div');
                item.className = 'node-icon-item';
                
                // Header with node name
                const header = document.createElement('div');
                header.className = 'node-icon-header';
                
                const nodeLabel = document.createElement('span');
                nodeLabel.className = 'node-id';
                nodeLabel.textContent = `${node.name || 'Node'} (${nodeId.slice(0, 8)}...)`;
                nodeLabel.title = `Node ID: ${nodeId}`;
                
                header.appendChild(nodeLabel);
                
                // Controls container
                const controlsContainer = document.createElement('div');
                controlsContainer.className = 'node-icon-controls';
                
                // First row: Preview and input
                const firstRow = document.createElement('div');
                firstRow.className = 'node-icon-controls-row';
                
                // Icon preview
                const iconPreview = document.createElement('div');
                iconPreview.className = 'icon-preview';
                
                const customImage = texturePacks.custom.customNodeImages.get(nodeId);
                const customIcon = texturePacks.custom.customNodeIcons.get(nodeId);
                
                if (customImage) {
                    const img = document.createElement('img');
                    img.src = customImage;
                    img.title = 'Custom image';
                    iconPreview.appendChild(img);
                } else if (customIcon) {
                    iconPreview.textContent = customIcon;
                } else {
                    iconPreview.textContent = texturePacks.custom.nodeIcons;
                }
                
                // Text/Emoji input
                const iconInput = document.createElement('input');
                iconInput.type = 'text';
                iconInput.className = 'icon-input';
                iconInput.placeholder = '🔵';
                iconInput.value = customIcon || '';
                iconInput.addEventListener('input', (e) => {
                    const icon = e.target.value;
                    if (icon) {
                        texturePacks.custom.customNodeIcons.set(nodeId, icon);
                    } else {
                        texturePacks.custom.customNodeIcons.delete(nodeId);
                    }
                    // Clear image if text is set
                    if (icon && texturePacks.custom.customNodeImages.has(nodeId)) {
                        texturePacks.custom.customNodeImages.delete(nodeId);
                    }
                    updateNodeIconPreview(nodeId, iconPreview);
                    if (currentTexturePack === 'custom') {
                        draw();
                    }
                });
                
                // Second row: Buttons
                const secondRow = document.createElement('div');
                secondRow.className = 'node-icon-controls-row';
                secondRow.style.justifyContent = 'flex-end';
                
                // Hidden file input for image upload
                const fileInput = document.createElement('input');
                fileInput.type = 'file';
                fileInput.accept = 'image/*';
                fileInput.className = 'hidden-file-input';
                fileInput.addEventListener('change', (e) => {
                    handleNodeImageUpload(e, nodeId, iconPreview);
                });
                
                // Upload button
                const uploadBtn = document.createElement('button');
                uploadBtn.className = 'icon-upload-btn';
                uploadBtn.textContent = '📁 Image';
                uploadBtn.title = 'Upload image for this node';
                uploadBtn.addEventListener('click', () => {
                    fileInput.click();
                });
                
                // Clear button
                const clearBtn = document.createElement('button');
                clearBtn.className = 'icon-clear-btn';
                clearBtn.textContent = '×';
                clearBtn.title = 'Clear custom icon/image';
                clearBtn.addEventListener('click', () => {
                    texturePacks.custom.customNodeIcons.delete(nodeId);
                    texturePacks.custom.customNodeImages.delete(nodeId);
                    iconInput.value = '';
                    updateNodeIconPreview(nodeId, iconPreview);
                    if (currentTexturePack === 'custom') {
                        draw();
                    }
                });
                
                // Assemble the layout
                firstRow.appendChild(iconPreview);
                firstRow.appendChild(iconInput);
                
                secondRow.appendChild(uploadBtn);
                secondRow.appendChild(clearBtn);
                secondRow.appendChild(fileInput);
                
                controlsContainer.appendChild(firstRow);
                controlsContainer.appendChild(secondRow);
                
                item.appendChild(header);
                item.appendChild(controlsContainer);
                container.appendChild(item);
            });
        }
        
        function updateNodeIconPreview(nodeId, previewElement) {
            previewElement.innerHTML = '';
            
            const customImage = texturePacks.custom.customNodeImages.get(nodeId);
            const customIcon = texturePacks.custom.customNodeIcons.get(nodeId);
            
            if (customImage) {
                const img = document.createElement('img');
                img.src = customImage;
                img.title = 'Custom image';
                previewElement.appendChild(img);
            } else if (customIcon) {
                previewElement.textContent = customIcon;
            } else {
                previewElement.textContent = texturePacks.custom.nodeIcons;
            }
        }
        
        function handleNodeImageUpload(event, nodeId, previewElement) {
            const file = event.target.files[0];
            if (file && file.type.startsWith('image/')) {
                const reader = new FileReader();
                reader.onload = (e) => {
                    const imageData = e.target.result;
                    texturePacks.custom.customNodeImages.set(nodeId, imageData);
                    // Clear text icon when image is set
                    texturePacks.custom.customNodeIcons.delete(nodeId);
                    // Update preview
                    updateNodeIconPreview(nodeId, previewElement);
                    // Clear the text input
                    const iconInput = previewElement.parentElement.querySelector('.icon-input');
                    if (iconInput) iconInput.value = '';
                    // Redraw if custom texture pack is active
                    if (currentTexturePack === 'custom') {
                        draw();
                    }
                };
                reader.readAsDataURL(file);
            }
        }
        
        function toggleNodeIconsSection() {
            const section = document.getElementById('nodeIconSection');
            const toggleBtn = document.getElementById('toggleNodeIcons');
            
            if (section.classList.contains('collapsed')) {
                section.classList.remove('collapsed');
                toggleBtn.textContent = '▼';
                toggleBtn.title = 'Collapse node icons';
            } else {
                section.classList.add('collapsed');
                toggleBtn.textContent = '▶';
                toggleBtn.title = 'Expand node icons';
            }
        }
        
        function applyCustomTheme() {
            const pack = texturePacks.custom;
            
            if (pack.backgroundTheme === 'dark') {
                // Apply dark theme
                document.documentElement.style.setProperty('--surface-color', '#2d3748');
                document.documentElement.style.setProperty('--text-primary', '#f7fafc');
                document.documentElement.style.setProperty('--text-secondary', '#a0aec0');
                document.documentElement.style.setProperty('--border-color', '#4a5568');
                document.documentElement.style.setProperty('--background-color', '#1a202c');
            } else {
                // Apply light theme
                document.documentElement.style.setProperty('--surface-color', '#ffffff');
                document.documentElement.style.setProperty('--text-primary', '#1e293b');
                document.documentElement.style.setProperty('--text-secondary', '#64748b');
                document.documentElement.style.setProperty('--border-color', '#e2e8f0');
                document.documentElement.style.setProperty('--background-color', '#f8fafc');
            }
            
            updateCustomBackground();
            draw();
        }
        
        function saveCustomTexturePack() {
            // Save to localStorage for persistence
            const customPack = {
                backgroundImage: texturePacks.custom.backgroundImage,
                backgroundOpacity: texturePacks.custom.backgroundOpacity,
                backgroundTheme: texturePacks.custom.backgroundTheme,
                nodeIcons: texturePacks.custom.nodeIcons,
                customNodeIcons: Array.from(texturePacks.custom.customNodeIcons.entries()),
                customNodeImages: Array.from(texturePacks.custom.customNodeImages.entries())
            };
            
            localStorage.setItem('aranya-custom-texture-pack', JSON.stringify(customPack));
            
            // Also save current texture pack selection and other preferences
            const preferences = {
                currentTexturePack: currentTexturePack,
                showConnections: showConnections,
                viewportX: viewportX,
                viewportY: viewportY,
                viewportScale: viewportScale
            };
            
            localStorage.setItem('aranya-preferences', JSON.stringify(preferences));
            
            // Show confirmation
            const button = document.getElementById('saveCustomTexture');
            const originalText = button.textContent;
            button.textContent = 'Saved!';
            button.style.background = 'var(--success-color)';
            setTimeout(() => {
                button.textContent = originalText;
                button.style.background = '';
            }, 2000);
        }
        
        function loadCustomTexturePack() {
            const saved = localStorage.getItem('aranya-custom-texture-pack');
            if (saved) {
                try {
                    const customPack = JSON.parse(saved);
                    texturePacks.custom.backgroundImage = customPack.backgroundImage;
                    texturePacks.custom.backgroundOpacity = customPack.backgroundOpacity || 80;
                    texturePacks.custom.backgroundTheme = customPack.backgroundTheme || 'light';
                    texturePacks.custom.nodeIcons = customPack.nodeIcons || '🔵';
                    texturePacks.custom.customNodeIcons = new Map(customPack.customNodeIcons || []);
                    texturePacks.custom.customNodeImages = new Map(customPack.customNodeImages || []);
                    
                    // Update UI
                    document.getElementById('backgroundTheme').value = texturePacks.custom.backgroundTheme;
                    document.getElementById('backgroundOpacity').value = texturePacks.custom.backgroundOpacity;
                    document.getElementById('opacityValue').textContent = texturePacks.custom.backgroundOpacity + '%';
                    document.getElementById('defaultNodeIcon').value = texturePacks.custom.nodeIcons;
                    
                    customBackgroundImage = texturePacks.custom.backgroundImage;
                } catch (e) {
                    console.warn('Failed to load custom texture pack:', e);
                }
            }
        }
        
        function loadPreferences() {
            const saved = localStorage.getItem('aranya-preferences');
            if (saved) {
                try {
                    const preferences = JSON.parse(saved);
                    
                    // Restore viewport settings
                    if (preferences.viewportX !== undefined) viewportX = preferences.viewportX;
                    if (preferences.viewportY !== undefined) viewportY = preferences.viewportY;
                    if (preferences.viewportScale !== undefined) viewportScale = preferences.viewportScale;
                    if (preferences.showConnections !== undefined) showConnections = preferences.showConnections;
                    
                    // Restore texture pack selection
                    if (preferences.currentTexturePack && texturePacks[preferences.currentTexturePack]) {
                        setTimeout(() => {
                            setTexturePack(preferences.currentTexturePack);
                        }, 100); // Small delay to ensure DOM is ready
                    }
                } catch (e) {
                    console.warn('Failed to load preferences:', e);
                }
            }
        }
        
        function savePreferences() {
            const preferences = {
                currentTexturePack: currentTexturePack,
                showConnections: showConnections,
                viewportX: viewportX,
                viewportY: viewportY,
                viewportScale: viewportScale
            };
            
            localStorage.setItem('aranya-preferences', JSON.stringify(preferences));
        }
        
        // Auto-save preferences periodically and on certain actions
        let preferenceSaveTimeout = null;
        function schedulePreferenceSave() {
            if (preferenceSaveTimeout) clearTimeout(preferenceSaveTimeout);
            preferenceSaveTimeout = setTimeout(savePreferences, 1000); // Save 1 second after last change
        }
        
        function resetCustomTexturePack() {
            // Reset custom texture pack to defaults
            texturePacks.custom.backgroundImage = null;
            texturePacks.custom.backgroundOpacity = 80;
            texturePacks.custom.backgroundTheme = 'light';
            texturePacks.custom.nodeIcons = '🔵';
            texturePacks.custom.customNodeIcons.clear();
            texturePacks.custom.customNodeImages.clear();
            customBackgroundImage = null;
            
            // Reset UI
            document.getElementById('backgroundTheme').value = 'light';
            document.getElementById('backgroundUpload').value = '';
            document.getElementById('backgroundOpacity').value = 80;
            document.getElementById('opacityValue').textContent = '80%';
            document.getElementById('defaultNodeIcon').value = '🔵';
            
            // Clear localStorage
            localStorage.removeItem('aranya-custom-texture-pack');
            
            updateIndividualNodeIcons();
            if (currentTexturePack === 'custom') {
                applyCustomTheme();
            }
        }
        
        function setTexturePack(packName) {
            if (!texturePacks[packName]) return;
            
            currentTexturePack = packName;
            const pack = texturePacks[packName];
            
            // Update selected state
            document.querySelectorAll('.texture-option').forEach(option => {
                option.classList.remove('selected');
                if (option.dataset.theme === packName) {
                    option.classList.add('selected');
                }
            });
            
            // Apply theme to canvas viewport
            const viewport = document.querySelector('.canvas-viewport');
            
            if (packName === 'custom') {
                applyCustomTheme();
            } else {
                viewport.style.setProperty('--texture-bg', pack.bg);
                viewport.style.setProperty('--texture-pattern', pack.pattern);
                viewport.style.setProperty('--texture-size', pack.size);
                viewport.style.opacity = 1;
                
                // Update CSS variables for theme
                if (packName === 'dark') {
                    document.documentElement.style.setProperty('--surface-color', '#2d3748');
                    document.documentElement.style.setProperty('--text-primary', '#f7fafc');
                    document.documentElement.style.setProperty('--text-secondary', '#a0aec0');
                    document.documentElement.style.setProperty('--border-color', '#4a5568');
                    document.documentElement.style.setProperty('--background-color', '#1a1a1a');
                } else {
                    document.documentElement.style.setProperty('--surface-color', '#ffffff');
                    document.documentElement.style.setProperty('--text-primary', '#1e293b');
                    document.documentElement.style.setProperty('--text-secondary', '#64748b');
                    document.documentElement.style.setProperty('--border-color', '#e2e8f0');
                    document.documentElement.style.setProperty('--background-color', '#f8fafc');
                }
            }
            
            draw();
            
            // Auto-save preferences when texture pack changes
            schedulePreferenceSave();
        }
        let ws = null;
        let nodes = new Map();
        let connections = new Map();
        let teams = new Map();
        let canvas = document.getElementById('canvas');
        let ctx = canvas.getContext('2d');
        
        // Initialize canvas size
        function initializeCanvas() {
            const viewport = document.querySelector('.canvas-viewport');
            const rect = viewport.getBoundingClientRect();
            
            // Set canvas size to match viewport
            canvas.width = rect.width;
            canvas.height = rect.height;
            
            // Center the viewport initially if this is first initialization
            if (viewportX === 0 && viewportY === 0) {
                viewportX = (canvas.width - CANVAS_WIDTH * viewportScale) / 2;
                viewportY = (canvas.height - CANVAS_HEIGHT * viewportScale) / 2;
            }
            
            // Redraw after resize
            draw();
        }
        
        // Handle window resize
        window.addEventListener('resize', initializeCanvas);
        let dragNode = null;
        let showConnections = true;
        let contextMenuNode = null;
        let currentTool = 'select';
        let connectStart = null;
        let mousePos = { x: 0, y: 0 };
        let selectedTeamId = null;
        let recentMessages = [];
        let messageBubbles = new Map();
        let messagePollingInterval = null;
        let lastKnownMessages = new Map(); // nodeId-teamId -> Set of messageIds
        let messageSenders = new Map(); // message_id -> actual sender node_id
        
        // Viewport/camera variables for panning and zooming
        let viewportX = 0;
        let viewportY = 0;
        let viewportScale = 1;
        let isPanning = false;
        let lastPanX = 0;
        let lastPanY = 0;
        const CANVAS_WIDTH = 2000;  // Virtual canvas size
        const CANVAS_HEIGHT = 1500;

        function connectWebSocket() {
            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            ws = new WebSocket(`${protocol}//${window.location.host}/ws`);
            
            ws.onopen = function() {
                document.getElementById('status').textContent = 'Connected. Click to place nodes!';
                ws.send(JSON.stringify({ type: 'GetState' }));
                // Start message polling after connection
                setTimeout(startMessagePolling, 2000);
            };
            
            ws.onmessage = function(event) {
                const message = JSON.parse(event.data);
                handleWebSocketMessage(message);
            };
            
            ws.onclose = function() {
                document.getElementById('status').textContent = 'Disconnected. Attempting to reconnect...';
                stopMessagePolling();
                setTimeout(connectWebSocket, 1000);
            };
            
            ws.onerror = function(error) {
                console.error('WebSocket error:', error);
            };
        }

        function handleWebSocketMessage(message) {
            switch (message.type) {
                case 'State':
                    nodes.clear();
                    connections.clear();
                    teams.clear();
                    message.nodes.forEach(node => nodes.set(node.id, node));
                    message.connections.forEach(conn => connections.set(`${conn.from}-${conn.to}-${conn.team_id}`, conn));
                    message.teams.forEach(team => teams.set(team.id, team));
                    updateTeamsUI();
                    draw();
                    break;
                case 'NodeCreated':
                    nodes.set(message.node.id, message.node);
                    updateNodeSelectors();
                    // Update individual node icons if custom texture panel is open
                    if (document.getElementById('customTexturePanel').style.display !== 'none') {
                        updateIndividualNodeIcons();
                    }
                    draw();
                    break;
                case 'NodeDeleted':
                    nodes.delete(message.node_id);
                    // Remove from custom node icons and images
                    if (texturePacks.custom.customNodeIcons.has(message.node_id)) {
                        texturePacks.custom.customNodeIcons.delete(message.node_id);
                    }
                    if (texturePacks.custom.customNodeImages.has(message.node_id)) {
                        // Clean up image cache
                        const imageData = texturePacks.custom.customNodeImages.get(message.node_id);
                        nodeImageCache.delete(imageData);
                        texturePacks.custom.customNodeImages.delete(message.node_id);
                    }
                    updateTeamsUI();
                    updateNodeSelectors();
                    // Update individual node icons if custom texture panel is open
                    if (document.getElementById('customTexturePanel').style.display !== 'none') {
                        updateIndividualNodeIcons();
                    }
                    draw();
                    break;
                case 'NodeMoved':
                    const node = nodes.get(message.node_id);
                    if (node) {
                        node.position = message.position;
                        draw();
                    }
                    break;
                case 'NodeStatusChanged':
                    const statusNode = nodes.get(message.node_id);
                    if (statusNode) {
                        statusNode.status = message.status;
                        updateNodeSelectors(); // Update since only running nodes are shown
                        draw();
                    }
                    break;
                case 'NodeTeamsChanged':
                    const teamsNode = nodes.get(message.node_id);
                    if (teamsNode) {
                        teamsNode.teams = message.teams;
                        updateTeamsUI();
                        draw();
                    }
                    break;
                case 'SyncConnectionAdded':
                    connections.set(`${message.connection.from}-${message.connection.to}-${message.connection.team_id}`, message.connection);
                    draw();
                    break;
                case 'SyncConnectionRemoved':
                    connections.delete(`${message.from}-${message.to}-${message.team_id}`);
                    draw();
                    break;
                case 'TeamCreated':
                    const team = message.team;
                    const creatorNode = nodes.get(message.node_id);
                    if (creatorNode) {
                        creatorNode.teams = creatorNode.teams || [];
                        creatorNode.teams.push(team);
                        teams.set(team.id, {
                            id: team.id,
                            name: team.name,
                            owner_node_id: message.node_id,
                            members: [{ node_id: message.node_id, role: 'Owner' }]
                        });
                        updateTeamsUI();
                        draw();
                        
                        // Check if this is part of a quick setup
                        if (window.pendingQuickSetup) {
                            completeQuickSetup(team.id);
                        }
                    }
                    break;
                case 'TeamJoined':
                    const joinedNode = nodes.get(message.node_id);
                    if (joinedNode) {
                        joinedNode.teams = joinedNode.teams || [];
                        joinedNode.teams.push(message.team);
                        const existingTeam = teams.get(message.team.id);
                        if (existingTeam) {
                            existingTeam.members.push({ node_id: message.node_id, role: message.team.role || 'Member' });
                        }
                        updateTeamsUI();
                        draw();
                    }
                    break;
                case 'MessageSent':
                    // Track the actual sender for this message
                    messageSenders.set(message.message_id, message.node_id);
                    
                    // Add to recent messages
                    const sentMessage = {
                        id: message.message_id,
                        author_id: message.author_id,
                        author_name: getNodeName(message.author_id),
                        text: message.text,
                        timestamp: message.timestamp,
                        team_id: message.team_id
                    };
                    recentMessages.push(sentMessage);
                    updateRecentMessages();
                    const authorDisplay = sentMessage.author_name !== 'Unknown' ? 
                        sentMessage.author_name : 
                        sentMessage.author_id.substring(0, 8) + '...';
                    console.log(`📤 Message sent by ${authorDisplay}: "${sentMessage.text}"`);
                    
                    // Show bubble on sending node
                    const sendingNode = nodes.get(message.node_id);
                    if (sendingNode) {
                        showMessageBubble(message.node_id, sentMessage.text, authorDisplay);
                    }
                    break;
                case 'MessageReceived':
                    // Track messages per node-team to avoid duplicates
                    const nodeTeamKey = `${message.node_id}-${message.team_id}`;
                    if (!lastKnownMessages.has(nodeTeamKey)) {
                        lastKnownMessages.set(nodeTeamKey, new Set());
                    }
                    const knownMessageIds = lastKnownMessages.get(nodeTeamKey);
                    
                    // Only process if this is a new message for this node-team
                    if (!knownMessageIds.has(message.message_id)) {
                        knownMessageIds.add(message.message_id);
                        
                        // Get the actual sender ID from our tracking map
                        const actualSenderId = messageSenders.get(message.message_id);
                        const authorName = actualSenderId ? getNodeName(actualSenderId) : 'Unknown';
                        const authorDisplay = authorName !== 'Unknown' ? 
                            authorName : 
                            (actualSenderId || message.author_id).substring(0, 8) + '...';
                        
                        const receivedMessage = {
                            id: message.message_id,
                            author_id: actualSenderId || message.author_id,
                            author_name: authorDisplay,
                            text: message.text,
                            timestamp: message.timestamp,
                            team_id: message.team_id
                        };
                        
                        // Add to recent messages if not already there globally
                        if (!recentMessages.some(m => m.id === receivedMessage.id)) {
                            recentMessages.push(receivedMessage);
                            updateRecentMessages();
                        }
                        
                        // Show bubble on receiving node 
                        const receivingNode = nodes.get(message.node_id);
                        if (receivingNode) {
                            console.log(`📨 Node ${receivingNode.name} received message: "${receivedMessage.text}" from ${authorDisplay}`);
                            showMessageBubble(message.node_id, receivedMessage.text, authorDisplay);
                        }
                    }
                    break;
            }
        }

        function draw() {
            ctx.clearRect(0, 0, canvas.width, canvas.height);
            
            // Save context and apply viewport transformations
            ctx.save();
            ctx.translate(viewportX, viewportY);
            ctx.scale(viewportScale, viewportScale);
            
            // Draw custom background image if available (game map style)
            if (currentTexturePack === 'custom' && customBackgroundImage && customBackgroundImage.complete) {
                const pack = texturePacks.custom;
                const opacity = pack.backgroundOpacity / 100;
                
                ctx.save();
                ctx.globalAlpha = opacity;
                
                // Draw background image to cover the entire virtual canvas area
                // This makes the background behave like a game map that moves/zooms with the viewport
                ctx.drawImage(
                    customBackgroundImage,
                    0, 0,  // Start at virtual canvas origin
                    CANVAS_WIDTH, CANVAS_HEIGHT  // Cover entire virtual canvas
                );
                
                ctx.restore();
            }
            
            // Draw virtual canvas boundaries
            ctx.strokeStyle = '#ccc';
            ctx.lineWidth = 2 / viewportScale;
            ctx.setLineDash([10 / viewportScale, 5 / viewportScale]);
            ctx.strokeRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);
            ctx.setLineDash([]);
            
            // Draw connections
            if (showConnections) {
                connections.forEach(conn => {
                    const fromNode = nodes.get(conn.from);
                    const toNode = nodes.get(conn.to);
                    if (fromNode && toNode) {
                        const color = conn.status === 'Connected' ? '#4CAF50' : 
                                     conn.status === 'Failed' ? '#F44336' : '#FF9800';
                        // Draw arrow from source (to) to syncer (from) to show data flow direction
                        drawArrow(toNode, fromNode, color);
                    }
                });
            }
            
            // Draw connection preview when in connect mode
            if (currentTool === 'connect' && connectStart) {
                const startNode = nodes.get(connectStart);
                if (startNode) {
                    ctx.save();
                    ctx.strokeStyle = '#666';
                    ctx.lineWidth = 2;
                    ctx.setLineDash([5, 5]);
                    ctx.beginPath();
                    ctx.moveTo(startNode.position.x, startNode.position.y);
                    ctx.lineTo(mousePos.x, mousePos.y);
                    ctx.stroke();
                    ctx.restore();
                }
            }
            
            // Draw nodes
            nodes.forEach(node => {
                drawNode(node);
            });
            
            // Restore context
            ctx.restore();
        }

        function drawNode(node) {
            const x = node.position.x;
            const y = node.position.y;
            const radius = 30;
            
            // Set color based on status
            let color = '#9E9E9E'; // stopped
            if (node.status === 'Running') color = '#4CAF50';
            else if (node.status === 'Starting') color = '#FF9800';
            else if (typeof node.status === 'object' && node.status.Error) color = '#F44336';
            
            // Draw main node circle
            ctx.fillStyle = color;
            ctx.beginPath();
            ctx.arc(x, y, radius, 0, 2 * Math.PI);
            ctx.fill();
            
            // Draw role-based outlines
            if (node.teams && node.teams.length > 0) {
                // Sort teams by role priority for consistent layering
                const sortedTeams = [...node.teams].sort((a, b) => {
                    const roleOrder = { 'Owner': 0, 'Admin': 1, 'Operator': 2, 'Member': 3 };
                    return roleOrder[a.role] - roleOrder[b.role];
                });
                
                sortedTeams.forEach((team, index) => {
                    // Use different colors for different roles
                    let teamColor = '#2196F3'; // Member
                    if (team.role === 'Owner') teamColor = '#FF5722';
                    else if (team.role === 'Admin') teamColor = '#FF9800';
                    else if (team.role === 'Operator') teamColor = '#9C27B0';
                    
                    // Draw outline ring - each role gets its own ring
                    ctx.strokeStyle = teamColor;
                    ctx.lineWidth = 4;
                    ctx.beginPath();
                    ctx.arc(x, y, radius + 2 + (index * 5), 0, 2 * Math.PI);
                    ctx.stroke();
                });
            } else {
                // Default outline for nodes without teams
                ctx.strokeStyle = '#333';
                ctx.lineWidth = 3;
                ctx.beginPath();
                ctx.arc(x, y, radius, 0, 2 * Math.PI);
                ctx.stroke();
            }
            
            // Draw node icon or name based on texture pack
            const pack = texturePacks[currentTexturePack];
            let nodeIcon = pack.nodeIcons;
            let nodeImage = null;
            
            // Check for individual node customization in custom texture pack
            if (currentTexturePack === 'custom') {
                // Check for uploaded image first (highest priority)
                if (pack.customNodeImages && pack.customNodeImages.has(node.id)) {
                    const imageData = pack.customNodeImages.get(node.id);
                    nodeImage = getOrCreateImageElement(imageData);
                }
                // Otherwise check for custom text/emoji icon
                else if (pack.customNodeIcons && pack.customNodeIcons.has(node.id)) {
                    nodeIcon = pack.customNodeIcons.get(node.id);
                }
            }
            
            if (nodeImage && nodeImage.complete) {
                // Draw uploaded image
                const imageSize = 40; // Size of the image icon
                ctx.save();
                
                // Create circular clipping path
                ctx.beginPath();
                ctx.arc(x, y, imageSize / 2, 0, 2 * Math.PI);
                ctx.clip();
                
                // Draw image centered
                ctx.drawImage(
                    nodeImage, 
                    x - imageSize / 2, 
                    y - imageSize / 2, 
                    imageSize, 
                    imageSize
                );
                
                ctx.restore();
                
                // Draw name below image
                ctx.fillStyle = currentTexturePack === 'dark' ? 'white' : '#333';
                ctx.font = 'bold 12px Arial';
                ctx.textAlign = 'center';
                ctx.fillText(node.name, x, y + 50);
            } else if (nodeIcon && nodeIcon !== '🔵') {
                // Draw text/emoji icon
                ctx.font = '24px Arial';
                ctx.textAlign = 'center';
                ctx.fillStyle = currentTexturePack === 'dark' ? 'white' : '#333';
                ctx.fillText(nodeIcon, x, y + 8);
                
                // Draw name below
                ctx.fillStyle = currentTexturePack === 'dark' ? 'white' : '#333';
                ctx.font = 'bold 12px Arial';
                ctx.fillText(node.name, x, y + 50);
            } else {
                // Draw name in center (default behavior)
                ctx.fillStyle = 'white';
                ctx.font = 'bold 14px Arial';
                ctx.textAlign = 'center';
                ctx.fillText(node.name, x, y + 4);
            }
        }
        
        function getOrCreateImageElement(imageData) {
            // Check cache first
            if (nodeImageCache.has(imageData)) {
                return nodeImageCache.get(imageData);
            }
            
            // Create new image element
            const img = new Image();
            img.src = imageData;
            nodeImageCache.set(imageData, img);
            
            // Redraw when image loads
            img.onload = () => {
                if (currentTexturePack === 'custom') {
                    draw();
                }
            };
            
            return img;
        }

        function drawArrow(fromNode, toNode, color = '#666') {
            const headlen = 10;
            const dx = toNode.position.x - fromNode.position.x;
            const dy = toNode.position.y - fromNode.position.y;
            const angle = Math.atan2(dy, dx);
            
            // Calculate radius including outline rings
            const baseRadius = 30;
            const fromRadius = baseRadius + (fromNode.teams && fromNode.teams.length > 0 ? 
                2 + (fromNode.teams.length - 1) * 5 + 4 : 0);
            const toRadius = baseRadius + (toNode.teams && toNode.teams.length > 0 ? 
                2 + (toNode.teams.length - 1) * 5 + 4 : 0);
            
            // Adjust start and end points to node edges (including outlines)
            const dist = Math.sqrt(dx * dx + dy * dy);
            const startX = fromNode.position.x + (fromRadius * dx) / dist;
            const startY = fromNode.position.y + (fromRadius * dy) / dist;
            const endX = toNode.position.x - (toRadius * dx) / dist;
            const endY = toNode.position.y - (toRadius * dy) / dist;
            
            ctx.strokeStyle = color;
            ctx.lineWidth = 2;
            ctx.beginPath();
            ctx.moveTo(startX, startY);
            ctx.lineTo(endX, endY);
            ctx.stroke();
            
            // Draw arrowhead
            ctx.beginPath();
            ctx.moveTo(endX, endY);
            ctx.lineTo(endX - headlen * Math.cos(angle - Math.PI / 6), endY - headlen * Math.sin(angle - Math.PI / 6));
            ctx.moveTo(endX, endY);
            ctx.lineTo(endX - headlen * Math.cos(angle + Math.PI / 6), endY - headlen * Math.sin(angle + Math.PI / 6));
            ctx.stroke();
        }

        // Coordinate transformation functions
        function screenToWorld(screenX, screenY) {
            return {
                x: (screenX - viewportX) / viewportScale,
                y: (screenY - viewportY) / viewportScale
            };
        }
        
        function worldToScreen(worldX, worldY) {
            return {
                x: worldX * viewportScale + viewportX,
                y: worldY * viewportScale + viewportY
            };
        }
        
        function getNodeAt(screenX, screenY) {
            for (let [id, node] of nodes) {
                // Convert node world position to screen position
                const screenPos = worldToScreen(node.position.x, node.position.y);
                const dx = screenX - screenPos.x;
                const dy = screenY - screenPos.y;
                
                // Account for larger hit area due to role outline rings, scaled by viewport scale
                const baseRadius = 30 * viewportScale;
                const ringOffset = (node.teams ? Math.max(0, (node.teams.length - 1) * 5) + 6 : 0) * viewportScale;
                const maxRadius = baseRadius + ringOffset;
                
                if (dx * dx + dy * dy <= maxRadius * maxRadius) {
                    return id;
                }
            }
            return null;
        }

        canvas.addEventListener('click', function(e) {
            const rect = canvas.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            
            console.log(`Canvas click: screen(${x}, ${y}), canvas size(${canvas.width}, ${canvas.height}), viewport(${viewportX}, ${viewportY}), scale(${viewportScale})`);
            
            const nodeId = getNodeAt(x, y);
            if (nodeId) {
                console.log(`Hit node: ${nodeId}`);
            }
            
            if (currentTool === 'place') {
                if (!nodeId) {
                    const worldPos = screenToWorld(x, y);
                    console.log(`World position: (${worldPos.x}, ${worldPos.y})`);
                    // Clamp to virtual canvas bounds
                    const clampedX = Math.max(50, Math.min(CANVAS_WIDTH - 50, worldPos.x));
                    const clampedY = Math.max(50, Math.min(CANVAS_HEIGHT - 50, worldPos.y));
                    console.log(`Clamped position: (${clampedX}, ${clampedY})`);
                    
                    const nameInput = document.getElementById('nodeNameInput');
                    const name = nameInput.value.trim() || `Node${nodes.size + 1}`;
                    nameInput.value = '';
                    
                    ws.send(JSON.stringify({
                        type: 'CreateNode',
                        name: name,
                        position: { x: clampedX, y: clampedY }
                    }));
                }
            } else if (currentTool === 'connect') {
                if (nodeId) {
                    if (!connectStart) {
                        // Start connection
                        connectStart = nodeId;
                    } else if (connectStart !== nodeId) {
                        // Complete connection
                        if (!selectedTeamId) {
                            alert('Please select a team for sync operations first!');
                            return;
                        }
                        ws.send(JSON.stringify({
                            type: 'AddSyncConnection',
                            from: connectStart,
                            to: nodeId,
                            team_id: selectedTeamId
                        }));
                        connectStart = null;
                    } else {
                        // Clicked same node, cancel
                        connectStart = null;
                    }
                } else {
                    // Clicked empty space, cancel
                    connectStart = null;
                }
                draw();
            }
        });

        canvas.addEventListener('mousedown', function(e) {
            const rect = canvas.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            
            if (currentTool === 'select') {
                dragNode = getNodeAt(x, y);
                if (!dragNode && e.button === 0) { // Left click and no node = start panning
                    isPanning = true;
                    lastPanX = e.clientX;
                    lastPanY = e.clientY;
                    canvas.style.cursor = 'grabbing';
                }
            }
        });

        canvas.addEventListener('mousemove', function(e) {
            const rect = canvas.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            
            mousePos = screenToWorld(x, y); // Convert to world coordinates for consistency
            
            if (isPanning && currentTool === 'select') {
                // Handle panning
                const deltaX = e.clientX - lastPanX;
                const deltaY = e.clientY - lastPanY;
                viewportX += deltaX;
                viewportY += deltaY;
                lastPanX = e.clientX;
                lastPanY = e.clientY;
                draw();
                schedulePreferenceSave();
            } else if (dragNode && currentTool === 'select') {
                // Handle node dragging
                const worldPos = screenToWorld(x, y);
                // Clamp to virtual canvas bounds
                const clampedX = Math.max(30, Math.min(CANVAS_WIDTH - 30, worldPos.x));
                const clampedY = Math.max(30, Math.min(CANVAS_HEIGHT - 30, worldPos.y));
                
                ws.send(JSON.stringify({
                    type: 'MoveNode',
                    node_id: dragNode,
                    position: { x: clampedX, y: clampedY }
                }));
            }
            
            // Update canvas cursor
            if (currentTool === 'select') {
                if (isPanning) {
                    canvas.style.cursor = 'grabbing';
                } else {
                    canvas.style.cursor = getNodeAt(x, y) ? 'move' : 'grab';
                }
            } else if (currentTool === 'place') {
                canvas.style.cursor = 'crosshair';
            } else if (currentTool === 'connect') {
                canvas.style.cursor = getNodeAt(x, y) ? 'pointer' : 'crosshair';
            }
            
            // Redraw if we're in connect mode to show preview line
            if (currentTool === 'connect' && connectStart) {
                draw();
            }
        });

        canvas.addEventListener('mouseup', function() {
            dragNode = null;
            isPanning = false;
            if (currentTool === 'select') {
                canvas.style.cursor = 'grab';
            }
        });

        // Prevent default right-click menu
        canvas.addEventListener('contextmenu', function(e) {
            e.preventDefault();
            const rect = canvas.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            
            const nodeId = getNodeAt(x, y);
            if (nodeId) {
                contextMenuNode = nodeId;
                const menu = document.getElementById('contextMenu');
                menu.style.display = 'block';
                menu.style.left = e.clientX + 'px';
                menu.style.top = e.clientY + 'px';
            }
        });

        // Mouse wheel for zooming
        canvas.addEventListener('wheel', function(e) {
            e.preventDefault();
            
            const rect = canvas.getBoundingClientRect();
            const mouseX = e.clientX - rect.left;
            const mouseY = e.clientY - rect.top;
            
            // Zoom factor
            const zoomFactor = e.deltaY > 0 ? 0.9 : 1.1;
            const newScale = viewportScale * zoomFactor;
            
            // Limit zoom range
            if (newScale < 0.1 || newScale > 3.0) return;
            
            // Zoom towards mouse position
            const worldPosBeforeZoom = screenToWorld(mouseX, mouseY);
            viewportScale = newScale;
            const worldPosAfterZoom = screenToWorld(mouseX, mouseY);
            
            // Adjust viewport to keep mouse position stable
            viewportX += (worldPosAfterZoom.x - worldPosBeforeZoom.x) * viewportScale;
            viewportY += (worldPosAfterZoom.y - worldPosBeforeZoom.y) * viewportScale;
            
            updateZoomDisplay();
            draw();
            schedulePreferenceSave();
        });

        // Hide context menu on click elsewhere
        document.addEventListener('click', function(e) {
            const menu = document.getElementById('contextMenu');
            if (!menu.contains(e.target)) {
                menu.style.display = 'none';
                contextMenuNode = null;
            }
        });

        function clearAll() {
            nodes.forEach((node, id) => {
                ws.send(JSON.stringify({ type: 'DeleteNode', node_id: id }));
            });
        }

        function toggleConnections() {
            showConnections = !showConnections;
            draw();
            schedulePreferenceSave();
        }
        
        function resetView() {
            viewportX = 0;
            viewportY = 0;
            viewportScale = 1;
            updateZoomDisplay();
            draw();
            schedulePreferenceSave();
        }
        
        function updateZoomDisplay() {
            document.getElementById('zoomLevel').textContent = Math.round(viewportScale * 100) + '%';
        }

        function deleteSelectedNode() {
            if (contextMenuNode) {
                ws.send(JSON.stringify({ type: 'DeleteNode', node_id: contextMenuNode }));
                document.getElementById('contextMenu').style.display = 'none';
                contextMenuNode = null;
            }
        }

        function removeSyncPeers() {
            if (contextMenuNode) {
                // Remove all connections where this node is involved
                const toRemove = [];
                connections.forEach((conn, key) => {
                    if (conn.from === contextMenuNode || conn.to === contextMenuNode) {
                        toRemove.push({ from: conn.from, to: conn.to, team_id: conn.team_id });
                    }
                });
                
                toRemove.forEach(({ from, to, team_id }) => {
                    ws.send(JSON.stringify({ type: 'RemoveSyncConnection', from, to, team_id }));
                });
                
                document.getElementById('contextMenu').style.display = 'none';
            }
        }

        function viewNodeInfo() {
            if (contextMenuNode) {
                const node = nodes.get(contextMenuNode);
                if (node) {
                    // Count connections
                    let incomingCount = 0;
                    let outgoingCount = 0;
                    connections.forEach(conn => {
                        if (conn.to === contextMenuNode) incomingCount++;
                        if (conn.from === contextMenuNode) outgoingCount++;
                    });
                    
                    alert(`Node Info:\nName: ${node.name}\nStatus: ${JSON.stringify(node.status)}\nDaemon Port: ${node.daemon_port}\nREST Port: ${node.rest_port}\n\nConnections:\nIncoming: ${incomingCount}\nOutgoing: ${outgoingCount}`);
                }
                document.getElementById('contextMenu').style.display = 'none';
            }
        }

        function selectTool(tool) {
            currentTool = tool;
            connectStart = null;
            
            // Update active button states
            document.querySelectorAll('.controls .btn').forEach(btn => {
                btn.classList.remove('active');
            });
            document.getElementById(tool + 'Tool').classList.add('active'); // Reset connection state
            
            // Update button states
            document.querySelectorAll('.tool-button').forEach(btn => {
                btn.classList.remove('active');
            });
            document.getElementById(tool + 'Tool').classList.add('active');
            
            // Update instructions
            const header = document.querySelector('.header p');
            if (tool === 'select') {
                header.textContent = 'Click and drag to move nodes. Right-click for options.';
            } else if (tool === 'place') {
                header.textContent = 'Click on empty space to place a new node.';
            } else if (tool === 'connect') {
                header.textContent = 'Click on receiver node, then click on source node. Arrow shows data flow direction.';
            }
            
            draw();
        }

        function updateTeamsUI() {
            // Update team selector for sync operations
            const selector = document.getElementById('teamSelector');
            const currentValue = selector.value;
            selector.innerHTML = '<option value="">-- Select Team for Sync --</option>';
            
            teams.forEach(team => {
                const option = document.createElement('option');
                option.value = team.id;
                option.textContent = team.name;
                selector.appendChild(option);
            });
            
            // Restore selection if still valid
            if (currentValue && teams.has(currentValue)) {
                selector.value = currentValue;
                selectedTeamId = currentValue;
            }
            
            // Update message team selector
            const messageSelector = document.getElementById('messageTeamSelector');
            const currentMessageValue = messageSelector.value;
            messageSelector.innerHTML = '<option value="">-- Select Team to Send Message --</option>';
            
            teams.forEach(team => {
                const option = document.createElement('option');
                option.value = team.id;
                option.textContent = team.name;
                messageSelector.appendChild(option);
            });
            
            // Restore selection if still valid
            if (currentMessageValue && teams.has(currentMessageValue)) {
                messageSelector.value = currentMessageValue;
            }
            
            // Update message sender selector
            updateMessageSenderSelector();
            
            // Update send button state
            updateSendButtonState();
            
            // Update available teams selector for joining
            const availableTeamsSelector = document.getElementById('availableTeamsSelector');
            const currentAvailableValue = availableTeamsSelector.value;
            availableTeamsSelector.innerHTML = '<option value="">-- Select Team to Join --</option>';
            
            teams.forEach(team => {
                const option = document.createElement('option');
                option.value = team.id;
                option.textContent = `${team.name} (Owner: ${getNodeName(team.owner_node_id)})`;
                availableTeamsSelector.appendChild(option);
            });
            
            // Restore selection if still valid
            if (currentAvailableValue && teams.has(currentAvailableValue)) {
                availableTeamsSelector.value = currentAvailableValue;
            }
            
            // Update node selectors
            updateNodeSelectors();
            
            // Update teams list
            const teamsList = document.getElementById('teamsList');
            teamsList.innerHTML = '';
            
            if (teams.size === 0) {
                teamsList.innerHTML = '<p style="color: #999; font-style: italic;">No teams created yet</p>';
            } else {
                teams.forEach(team => {
                    const teamDiv = document.createElement('div');
                    teamDiv.className = 'team-item';
                    
                    // Find all nodes in this team
                    const teamNodeNames = [];
                    nodes.forEach(node => {
                        if (node.teams && node.teams.some(t => t.id === team.id)) {
                            const role = node.teams.find(t => t.id === team.id)?.role || 'Member';
                            teamNodeNames.push(`${node.name} (${role})`);
                        }
                    });
                    
                    teamDiv.innerHTML = `
                        <strong>${team.name}</strong> (${team.id.substring(0, 8)}...)
                        <div class="team-members">
                            Owner: ${getNodeName(team.owner_node_id)}<br>
                            Members: ${teamNodeNames.length}<br>
                            ${teamNodeNames.length > 0 ? 
                                `<small>${teamNodeNames.join(', ')}</small>` : 
                                '<small>No members yet</small>'
                            }
                        </div>
                    `;
                    teamsList.appendChild(teamDiv);
                });
            }
            
            // Update selected team info
            updateSelectedTeamInfo();
        }

        function updateNodeSelectors() {
            // Update node selector for team creation
            const nodeSelector = document.getElementById('nodeSelector');
            const currentNodeValue = nodeSelector.value;
            nodeSelector.innerHTML = '<option value="">-- Select Node --</option>';
            
            // Update join node selector
            const joinNodeSelector = document.getElementById('joinNodeSelector');
            const currentJoinNodeValue = joinNodeSelector.value;
            joinNodeSelector.innerHTML = '<option value="">-- Select Node --</option>';
            
            nodes.forEach(node => {
                // Add to create team selector (only running nodes)
                if (node.status === 'Running') {
                    const option1 = document.createElement('option');
                    option1.value = node.id;
                    option1.textContent = `${node.name} (${node.status})`;
                    nodeSelector.appendChild(option1);
                    
                    const option2 = document.createElement('option');
                    option2.value = node.id;
                    option2.textContent = `${node.name} (${node.status})`;
                    joinNodeSelector.appendChild(option2);
                }
            });
            
            // Restore selections if still valid
            if (currentNodeValue && nodes.has(currentNodeValue)) {
                nodeSelector.value = currentNodeValue;
            }
            if (currentJoinNodeValue && nodes.has(currentJoinNodeValue)) {
                joinNodeSelector.value = currentJoinNodeValue;
            }
        }

        function selectTeam() {
            const selector = document.getElementById('teamSelector');
            selectedTeamId = selector.value;
            updateSelectedTeamInfo();
        }

        function updateSelectedTeamInfo() {
            const info = document.getElementById('selectedTeamInfo');
            if (selectedTeamId) {
                const team = teams.get(selectedTeamId);
                if (team) {
                    info.textContent = `Selected: ${team.name}`;
                    info.className = '';
                } else {
                    info.textContent = 'Selected team not found';
                    info.className = 'no-team-selected';
                }
            } else {
                info.textContent = 'No team selected for sync operations';
                info.className = 'no-team-selected';
            }
        }

        function getNodeName(nodeId) {
            const node = nodes.get(nodeId);
            return node ? node.name : 'Unknown';
        }

        function createTeamForNode() {
            if (contextMenuNode) {
                const teamName = prompt('Enter team name:');
                if (teamName && teamName.trim()) {
                    ws.send(JSON.stringify({
                        type: 'CreateTeam',
                        node_id: contextMenuNode,
                        team_name: teamName.trim()
                    }));
                }
                document.getElementById('contextMenu').style.display = 'none';
            }
        }

        function joinTeamDialog() {
            if (contextMenuNode) {
                if (teams.size === 0) {
                    alert('No teams available to join. Create a team first.');
                    return;
                }
                
                let teamOptions = '';
                teams.forEach(team => {
                    teamOptions += `${team.name} (${team.id.substring(0, 8)}...)\n`;
                });
                
                const teamId = prompt(`Available teams:\n${teamOptions}\nEnter team ID to join:`);
                if (teamId && teamId.trim()) {
                    const team = teams.get(teamId.trim());
                    if (team) {
                        ws.send(JSON.stringify({
                            type: 'JoinTeam',
                            node_id: contextMenuNode,
                            team_id: teamId.trim(),
                            owner_node_id: team.owner_node_id
                        }));
                    } else {
                        alert('Team not found!');
                    }
                }
                document.getElementById('contextMenu').style.display = 'none';
            }
        }

        function createTeamFromUI() {
            const nodeSelector = document.getElementById('nodeSelector');
            const teamNameInput = document.getElementById('teamNameInput');
            
            const nodeId = nodeSelector.value;
            const teamName = teamNameInput.value.trim();
            
            if (!nodeId) {
                alert('Please select a node to create the team.');
                return;
            }
            
            if (!teamName) {
                alert('Please enter a team name.');
                return;
            }
            
            ws.send(JSON.stringify({
                type: 'CreateTeam',
                node_id: nodeId,
                team_name: teamName
            }));
            
            // Clear the input
            teamNameInput.value = '';
        }

        function joinTeamFromUI() {
            const joinNodeSelector = document.getElementById('joinNodeSelector');
            const availableTeamsSelector = document.getElementById('availableTeamsSelector');
            
            const nodeId = joinNodeSelector.value;
            const teamId = availableTeamsSelector.value;
            
            if (!nodeId) {
                alert('Please select a node to join the team.');
                return;
            }
            
            if (!teamId) {
                alert('Please select a team to join.');
                return;
            }
            
            const team = teams.get(teamId);
            if (!team) {
                alert('Selected team not found.');
                return;
            }
            
            ws.send(JSON.stringify({
                type: 'JoinTeam',
                node_id: nodeId,
                team_id: teamId,
                owner_node_id: team.owner_node_id
            }));
        }

        function addAllNodesToSelectedTeam() {
            if (!selectedTeamId) {
                alert('Please select a team first from the Sync Operations dropdown.');
                return;
            }

            const team = teams.get(selectedTeamId);
            if (!team) {
                alert('Selected team not found.');
                return;
            }

            // Find nodes that are running but not already in this team
            const nodesToAdd = [];
            nodes.forEach(node => {
                if (node.status === 'Running') {
                    // Check if node is already in this team
                    const alreadyInTeam = node.teams && node.teams.some(t => t.id === selectedTeamId);
                    if (!alreadyInTeam) {
                        nodesToAdd.push(node);
                    }
                }
            });

            if (nodesToAdd.length === 0) {
                alert('All running nodes are already in this team.');
                return;
            }

            if (confirm(`Add ${nodesToAdd.length} nodes to team "${team.name}"?`)) {
                nodesToAdd.forEach(node => {
                    ws.send(JSON.stringify({
                        type: 'JoinTeam',
                        node_id: node.id,
                        team_id: selectedTeamId,
                        owner_node_id: team.owner_node_id
                    }));
                });
            }
        }

        function autoSyncTeamMembers() {
            if (!selectedTeamId) {
                alert('Please select a team first from the Sync Operations dropdown.');
                return;
            }

            const team = teams.get(selectedTeamId);
            if (!team) {
                alert('Selected team not found.');
                return;
            }

            // Find all nodes in this team
            const teamNodes = [];
            nodes.forEach(node => {
                if (node.teams && node.teams.some(t => t.id === selectedTeamId)) {
                    teamNodes.push(node);
                }
            });

            if (teamNodes.length < 2) {
                alert('Need at least 2 nodes in the team to create sync connections.');
                return;
            }

            let connectionCount = 0;
            const connectionsToCreate = [];

            // Create bidirectional sync connections between all team members
            for (let i = 0; i < teamNodes.length; i++) {
                for (let j = 0; j < teamNodes.length; j++) {
                    if (i !== j) {
                        const from = teamNodes[i].id;
                        const to = teamNodes[j].id;
                        
                        // Check if connection already exists
                        const connectionKey = `${from}-${to}-${selectedTeamId}`;
                        if (!connections.has(connectionKey)) {
                            connectionsToCreate.push({ from, to });
                            connectionCount++;
                        }
                    }
                }
            }

            if (connectionCount === 0) {
                alert('All team members are already fully synced.');
                return;
            }

            if (confirm(`Create ${connectionCount} sync connections between all team members?`)) {
                connectionsToCreate.forEach(({ from, to }) => {
                    ws.send(JSON.stringify({
                        type: 'AddSyncConnection',
                        from: from,
                        to: to,
                        team_id: selectedTeamId
                    }));
                });
            }
        }

        function syncAllMembersFromOwner() {
            if (!selectedTeamId) {
                alert('Please select a team first from the Sync Operations dropdown.');
                return;
            }

            const team = teams.get(selectedTeamId);
            if (!team) {
                alert('Selected team not found.');
                return;
            }

            // Find the owner node
            const ownerNode = nodes.get(team.owner_node_id);
            if (!ownerNode || ownerNode.status !== 'Running') {
                alert('Team owner node is not running. Cannot create sync connections.');
                return;
            }

            // Find all other team members (excluding owner)
            const memberNodes = [];
            nodes.forEach(node => {
                if (node.status === 'Running' && 
                    node.id !== team.owner_node_id &&
                    node.teams && 
                    node.teams.some(t => t.id === selectedTeamId)) {
                    memberNodes.push(node);
                }
            });

            if (memberNodes.length === 0) {
                alert('No other running team members found. Add more nodes to the team first.');
                return;
            }

            const connectionCount = memberNodes.length;
            const ownerName = ownerNode.name;
            const memberNames = memberNodes.map(n => n.name).join(', ');

            if (confirm(`Make ${ownerName} (Owner) a sync peer for ${connectionCount} team members?\n\nThis will create sync connections FROM each member TO the owner:\n${memberNames}\n\nThis ensures all members receive commands from the owner.`)) {
                // Create sync connections from each member to the owner
                // This means each member will sync FROM the owner (receive owner's commands)
                memberNodes.forEach(memberNode => {
                    ws.send(JSON.stringify({
                        type: 'AddSyncConnection',
                        from: memberNode.id,      // Member syncs FROM owner
                        to: ownerNode.id,         // TO the owner
                        team_id: selectedTeamId
                    }));
                });

                alert(`Creating ${connectionCount} sync connections from team members to owner ${ownerName}. This will help ensure all members receive owner commands for team management.`);
            }
        }

        function quickTeamSetup() {
            const runningNodes = [];
            nodes.forEach(node => {
                if (node.status === 'Running') {
                    runningNodes.push(node);
                }
            });

            if (runningNodes.length === 0) {
                alert('No running nodes available. Place and start some nodes first.');
                return;
            }

            const teamName = prompt('Enter team name for quick setup:');
            if (!teamName || !teamName.trim()) {
                return;
            }

            if (confirm(`Quick setup will:\n1. Create team "${teamName}" with ${runningNodes[0].name} as owner\n2. Add all ${runningNodes.length} nodes to the team\n3. Make owner sync peer for all members\n\nProceed?`)) {
                // Step 1: Create team with first node as owner
                const ownerNode = runningNodes[0];
                ws.send(JSON.stringify({
                    type: 'CreateTeam',
                    node_id: ownerNode.id,
                    team_name: teamName.trim()
                }));

                // Store the setup for completion after team is created
                window.pendingQuickSetup = {
                    teamName: teamName.trim(),
                    ownerNodeId: ownerNode.id,
                    allNodes: runningNodes
                };
            }
        }

        // Helper function to complete quick setup after team creation
        function completeQuickSetup(teamId) {
            const setup = window.pendingQuickSetup;
            if (!setup) return;

            // Step 2: Add remaining nodes to team
            const nodesToAdd = setup.allNodes.filter(node => node.id !== setup.ownerNodeId);
            nodesToAdd.forEach(node => {
                setTimeout(() => {
                    ws.send(JSON.stringify({
                        type: 'JoinTeam',
                        node_id: node.id,
                        team_id: teamId,
                        owner_node_id: setup.ownerNodeId
                    }));
                }, 500); // Small delay between requests
            });

            // Step 3: Create sync connections (with delay to let joins complete)
            setTimeout(() => {
                // Set the team as selected
                selectedTeamId = teamId;
                document.getElementById('teamSelector').value = teamId;
                updateSelectedTeamInfo();

                // Make owner sync peer for all members
                setTimeout(() => {
                    syncAllMembersFromOwner();
                }, 2000); // Wait for joins to complete
            }, 1000);

            // Clear pending setup
            delete window.pendingQuickSetup;
        }

        // Messaging functions
        function updateSendButtonState() {
            const messageSenderSelector = document.getElementById('messageSenderSelector');
            const messageTeamSelector = document.getElementById('messageTeamSelector');
            const messageInput = document.getElementById('messageInput');
            const sendBtn = document.getElementById('sendMessageBtn');
            
            const hasSender = messageSenderSelector.value !== '';
            const hasTeam = messageTeamSelector.value !== '';
            const hasMessage = messageInput.value.trim() !== '';
            
            sendBtn.disabled = !hasSender || !hasTeam || !hasMessage;
        }

        function updateMessageSenderSelector() {
            const senderSelector = document.getElementById('messageSenderSelector');
            const teamSelector = document.getElementById('messageTeamSelector');
            const currentSenderValue = senderSelector.value;
            
            senderSelector.innerHTML = '<option value="">-- Select Sender Node --</option>';
            
            const selectedTeamId = teamSelector.value;
            if (selectedTeamId) {
                // Show only nodes that are in the selected team and running
                nodes.forEach(node => {
                    if (node.status === 'Running' && 
                        node.teams && 
                        node.teams.some(t => t.id === selectedTeamId)) {
                        const option = document.createElement('option');
                        option.value = node.id;
                        option.textContent = `${node.name} (${node.teams.find(t => t.id === selectedTeamId)?.role || 'Member'})`;
                        senderSelector.appendChild(option);
                    }
                });
            }
            
            // Restore selection if still valid
            if (currentSenderValue && senderSelector.querySelector(`option[value="${currentSenderValue}"]`)) {
                senderSelector.value = currentSenderValue;
            }
            
            updateSendButtonState();
        }

        function sendMessageToNode() {
            if (!contextMenuNode) return;
            
            const node = nodes.get(contextMenuNode);
            if (!node || !node.teams || node.teams.length === 0) {
                alert('This node is not part of any team. Join a team first to send messages.');
                document.getElementById('contextMenu').style.display = 'none';
                return;
            }
            
            // Show available teams for this node
            let teamOptions = 'Available teams for this node:\n';
            node.teams.forEach(team => {
                teamOptions += `- ${team.name} (Role: ${team.role})\n`;
            });
            
            const message = prompt(teamOptions + '\nEnter your message:');
            if (message && message.trim()) {
                // Use the first team for simplicity
                const teamId = node.teams[0].id;
                sendMessageViaNode(contextMenuNode, teamId, message.trim());
            }
            
            document.getElementById('contextMenu').style.display = 'none';
        }

        function sendBroadcastMessage() {
            const messageSenderSelector = document.getElementById('messageSenderSelector');
            const messageTeamSelector = document.getElementById('messageTeamSelector');
            const messageInput = document.getElementById('messageInput');
            
            const senderNodeId = messageSenderSelector.value;
            const teamId = messageTeamSelector.value;
            const message = messageInput.value.trim();
            
            if (!senderNodeId || !teamId || !message) {
                return;
            }
            
            sendMessageViaNode(senderNodeId, teamId, message);
            messageInput.value = '';
            updateSendButtonState();
        }

        function sendMessageViaNode(nodeId, teamId, message) {
            ws.send(JSON.stringify({
                type: 'SendMessage',
                node_id: nodeId,
                team_id: teamId,
                message: message
            }));
        }

        function showMessageBubble(nodeId, message, authorName) {
            const node = nodes.get(nodeId);
            if (!node) return;
            
            // Create bubble element
            const bubble = document.createElement('div');
            bubble.className = 'message-bubble';
            bubble.innerHTML = `<strong>${authorName}:</strong> ${message}`;
            
            // Position bubble above node (convert world coordinates to screen)
            const rect = canvas.getBoundingClientRect();
            const screenPos = worldToScreen(node.position.x, node.position.y);
            bubble.style.left = (rect.left + screenPos.x - 100) + 'px';
            bubble.style.top = (rect.top + screenPos.y - 60) + 'px';
            
            document.body.appendChild(bubble);
            
            // Animate in
            setTimeout(() => {
                bubble.classList.add('show');
            }, 100);
            
            // Remove after 6 seconds (longer duration for easier visibility)
            setTimeout(() => {
                bubble.classList.remove('show');
                setTimeout(() => {
                    document.body.removeChild(bubble);
                }, 400);
            }, 6000);
        }

        function updateRecentMessages() {
            const container = document.getElementById('recentMessages');
            
            if (recentMessages.length === 0) {
                container.innerHTML = '<p style="color: var(--text-secondary); font-style: italic; text-align: center; margin: 1rem 0;">No messages yet</p>';
                return;
            }
            
            let html = '';
            recentMessages.slice(-10).forEach(msg => {
                const time = new Date(msg.timestamp * 1000).toLocaleTimeString();
                // Show author name if available, otherwise truncate ID
                const authorDisplay = msg.author_name !== 'Unknown' ? 
                    msg.author_name : 
                    msg.author_id.substring(0, 8) + '...';
                html += `
                    <div class="message-item">
                        <div class="message-author">${authorDisplay} <span class="message-time">${time}</span></div>
                        <div class="message-text">${msg.text}</div>
                    </div>
                `;
            });
            
            container.innerHTML = html;
            container.scrollTop = container.scrollHeight;
        }

        // Message polling functions
        function startMessagePolling() {
            if (messagePollingInterval) {
                clearInterval(messagePollingInterval);
            }
            
            // Poll every 250ms for real-time message display (much faster than 1s daemon sync)
            messagePollingInterval = setInterval(pollForNewMessages, 250);
            
            // Also poll immediately
            setTimeout(pollForNewMessages, 1000);
        }

        function stopMessagePolling() {
            if (messagePollingInterval) {
                clearInterval(messagePollingInterval);
                messagePollingInterval = null;
            }
        }

        function pollForNewMessages() {
            // Get all teams and their member nodes
            const teamNodeMap = new Map();
            let totalPolls = 0;
            
            teams.forEach(team => {
                const teamNodes = [];
                nodes.forEach(node => {
                    if (node.status === 'Running' && 
                        node.teams && 
                        node.teams.some(t => t.id === team.id)) {
                        teamNodes.push(node.id);
                    }
                });
                if (teamNodes.length > 0) {
                    teamNodeMap.set(team.id, teamNodes);
                }
            });

            // Poll messages for each node in each team
            teamNodeMap.forEach((nodeIds, teamId) => {
                nodeIds.forEach(nodeId => {
                    pollMessagesForNode(nodeId, teamId);
                    totalPolls++;
                });
            });
            
            // Debug: Log polling activity periodically
            if (totalPolls > 0 && Math.random() < 0.05) { // Log ~5% of the time to avoid spam
                console.log(`Polling ${totalPolls} node-team combinations for new messages`);
            }
        }

        function pollMessagesForNode(nodeId, teamId) {
            ws.send(JSON.stringify({
                type: 'PollMessages',
                node_id: nodeId,
                team_id: teamId
            }));
        }

        // Event listeners for message panel
        document.getElementById('messageSenderSelector').addEventListener('change', updateSendButtonState);
        document.getElementById('messageTeamSelector').addEventListener('change', function() {
            updateMessageSenderSelector();
            updateSendButtonState();
        });
        document.getElementById('messageInput').addEventListener('input', updateSendButtonState);
        document.getElementById('messageInput').addEventListener('keypress', function(e) {
            if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
                sendBroadcastMessage();
            }
        });

        // Initialize
        initializeCanvas();
        initializeTexturePacks();
        loadCustomTexturePack();
        loadPreferences();
        connectWebSocket();
    </script>
</body>
</html>"#)
}