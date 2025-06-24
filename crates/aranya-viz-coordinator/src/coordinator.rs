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
            height: 100vh;
            display: flex;
            flex-direction: column;
        }
        
        .container { 
            display: flex;
            flex-direction: column;
            height: 100vh;
            overflow: hidden;
        }
        
        .header { 
            display: flex;
            align-items: center;
            justify-content: space-between;
            padding: 0.75rem 1rem;
            background: var(--surface-color);
            box-shadow: var(--shadow-sm);
            border-bottom: 1px solid var(--border-color);
            flex-shrink: 0;
        }
        
        .header-left {
            display: flex;
            align-items: center;
            gap: 1rem;
        }
        
        .header h1 {
            margin: 0;
            color: var(--primary-color);
            font-size: 1.25rem;
            font-weight: 600;
        }
        
        .header p {
            margin: 0;
            color: var(--text-secondary);
            font-size: 0.75rem;
        }
        
        .main-content {
            display: flex;
            gap: 0;
            flex: 1;
            min-height: 0;
            position: relative;
            overflow: hidden;
        }
        
        .resizer {
            width: 4px;
            background: var(--border-color);
            cursor: col-resize;
            position: relative;
            transition: background-color 0.2s;
            flex-shrink: 0;
        }
        
        .resizer:hover {
            background: var(--primary-color);
        }
        
        .resizer::after {
            content: '';
            position: absolute;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            width: 2px;
            height: 30px;
            background: rgba(0, 0, 0, 0.1);
            border-radius: 1px;
        }
        
        .canvas-container { 
            background: var(--surface-color);
            overflow: hidden;
            position: relative;
            display: flex;
            flex-direction: column;
            flex: 1;
            min-width: 400px;
            height: 100%;
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
            background: rgba(255, 255, 255, 0.9);
            padding: 0.5rem 0.75rem;
            border-radius: var(--radius-sm);
            border: 1px solid var(--border-color);
            font-size: 0.75rem;
            color: var(--text-secondary);
            display: inline-block;
        }
        
        .notification-area {
            position: fixed;
            top: 20px;
            right: 20px;
            width: 350px;
            z-index: 10000;
            pointer-events: none;
        }
        
        .notification {
            background: var(--surface-color);
            border: 1px solid var(--border-color);
            border-radius: var(--radius-md);
            padding: 1rem;
            margin-bottom: 0.5rem;
            box-shadow: var(--shadow-lg);
            animation: slideIn 0.3s ease-out;
            pointer-events: auto;
            display: flex;
            align-items: flex-start;
            gap: 0.75rem;
        }
        
        .notification.success {
            border-left: 4px solid var(--success-color);
        }
        
        .notification.warning {
            border-left: 4px solid var(--warning-color);
        }
        
        .notification.error {
            border-left: 4px solid var(--error-color);
        }
        
        .notification.info {
            border-left: 4px solid var(--primary-color);
        }
        
        .notification-icon {
            font-size: 1.25rem;
            flex-shrink: 0;
        }
        
        .notification-content {
            flex: 1;
        }
        
        .notification-title {
            font-weight: 600;
            margin-bottom: 0.25rem;
        }
        
        .notification-message {
            font-size: 0.875rem;
            color: var(--text-secondary);
        }
        
        @keyframes slideIn {
            from {
                transform: translateX(100%);
                opacity: 0;
            }
            to {
                transform: translateX(0);
                opacity: 1;
            }
        }
        
        @keyframes fadeOut {
            from {
                opacity: 1;
            }
            to {
                opacity: 0;
            }
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
        
        .texture-pack-management {
            margin-top: 1.5rem;
            padding: 1rem;
            background: var(--background-color);
            border: 1px solid var(--border-color);
            border-radius: var(--radius-md);
        }
        
        .texture-pack-management h5 {
            margin: 0 0 1rem 0;
            color: var(--text-primary);
            font-size: 0.875rem;
            font-weight: 600;
        }
        
        .texture-pack-management h6 {
            margin: 1rem 0 0.5rem 0;
            color: var(--text-secondary);
            font-size: 0.75rem;
            font-weight: 600;
        }
        
        .pack-name-input {
            width: 100%;
            padding: 0.5rem;
            border: 1px solid var(--border-color);
            border-radius: var(--radius-sm);
            background: var(--surface-color);
            font-size: 0.875rem;
            color: var(--text-primary);
        }
        
        .pack-actions {
            display: flex;
            gap: 0.5rem;
            margin: 1rem 0;
        }
        
        .pack-actions .btn {
            flex: 1;
            padding: 0.5rem;
            font-size: 0.75rem;
        }
        
        .saved-packs-list {
            max-height: 200px;
            overflow-y: auto;
            border: 1px solid var(--border-color);
            border-radius: var(--radius-sm);
            background: var(--surface-color);
        }
        
        .saved-pack-item {
            display: flex;
            align-items: center;
            justify-content: space-between;
            padding: 0.75rem;
            border-bottom: 1px solid var(--border-color);
            transition: background-color 0.2s ease;
        }
        
        .saved-pack-item:last-child {
            border-bottom: none;
        }
        
        .saved-pack-item:hover {
            background: var(--background-color);
        }
        
        .saved-pack-item.active {
            background: #eff6ff;
            border-color: var(--primary-color);
        }
        
        .pack-info {
            flex: 1;
            min-width: 0;
        }
        
        .pack-name {
            font-weight: 600;
            color: var(--text-primary);
            font-size: 0.875rem;
            margin-bottom: 0.25rem;
        }
        
        .pack-details {
            font-size: 0.75rem;
            color: var(--text-secondary);
        }
        
        .pack-actions-buttons {
            display: flex;
            gap: 0.25rem;
        }
        
        .pack-action-btn {
            padding: 0.25rem 0.5rem;
            font-size: 0.7rem;
            border: 1px solid var(--border-color);
            border-radius: var(--radius-sm);
            background: var(--surface-color);
            color: var(--text-secondary);
            cursor: pointer;
            transition: all 0.2s ease;
        }
        
        .pack-action-btn:hover {
            background: var(--background-color);
            border-color: var(--primary-color);
            color: var(--primary-color);
        }
        
        .pack-action-btn.load {
            background: var(--accent-color);
            color: white;
            border-color: var(--accent-color);
        }
        
        .pack-action-btn.load:hover {
            background: var(--primary-color);
            border-color: var(--primary-color);
        }
        
        .pack-action-btn.delete {
            background: var(--error-color);
            color: white;
            border-color: var(--error-color);
        }
        
        .pack-action-btn.delete:hover {
            background: #dc2626;
            border-color: #dc2626;
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
            padding: 0;
            overflow-y: auto;
            overflow-x: hidden;
            min-width: 280px;
            max-width: 600px;
            flex-shrink: 0;
            height: 100%;
        }
        
        .drawer {
            margin-bottom: 1px;
            background: var(--surface-color);
            border-bottom: 1px solid var(--border-color);
        }
        
        .drawer-header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            padding: 0.75rem 1rem;
            background: var(--background-color);
            cursor: pointer;
            user-select: none;
            transition: background-color 0.2s ease;
        }
        
        .drawer-header:hover {
            background: #f3f4f6;
        }
        
        .drawer-header.active {
            background: var(--primary-color);
            color: white;
        }
        
        .drawer-header.active .drawer-arrow {
            opacity: 1;
        }
        
        .drawer-title {
            display: flex;
            align-items: center;
            gap: 0.5rem;
            font-size: 0.875rem;
            font-weight: 600;
        }
        
        .drawer-icon {
            width: 1.25rem;
            height: 1.25rem;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 1rem;
        }
        
        .drawer-arrow {
            width: 0.75rem;
            height: 0.75rem;
            transition: transform 0.2s ease;
            opacity: 0.6;
        }
        
        .drawer-header.expanded .drawer-arrow {
            transform: rotate(90deg);
        }
        
        .drawer-content {
            max-height: 0;
            overflow: hidden;
            transition: max-height 0.3s ease;
            background: var(--surface-color);
        }
        
        .drawer-content.expanded {
            max-height: 2000px;
        }
        
        .drawer-inner {
            padding: 1rem;
        }
        
        .drawer-inner h5 {
            margin: 0 0 0.75rem 0;
            color: var(--text-primary);
            font-size: 0.9375rem;
            font-weight: 600;
        }
        
        .drawer-inner h5:not(:first-child) {
            margin-top: 1.5rem;
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
        
        /* Access Control and Sync Configuration Styles */
        .team-overview {
            background: var(--background-color);
            border: 1px solid var(--border-color);
            border-radius: var(--radius-md);
            padding: 1rem;
            margin-top: 1rem;
            max-height: 300px;
            overflow-y: auto;
        }
        
        .roles-list {
            display: flex;
            flex-direction: column;
            gap: 0.5rem;
        }
        
        .role-item {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 0.5rem;
            background: var(--surface-color);
            border: 1px solid var(--border-color);
            border-radius: var(--radius-sm);
            font-size: 0.875rem;
        }
        
        .role-badge {
            padding: 0.25rem 0.5rem;
            border-radius: var(--radius-sm);
            font-size: 0.75rem;
            font-weight: 600;
            color: white;
        }
        
        .role-badge.owner {
            background: #8b5cf6;
        }
        
        .role-badge.admin {
            background: #3b82f6;
        }
        
        .role-badge.operator {
            background: #10b981;
        }
        
        .role-badge.member {
            background: #6b7280;
        }
        
        .team-overview {
            margin-top: 1rem;
        }
        
        .roles-list {
            max-height: 300px;
            overflow-y: auto;
        }
        
        .role-item {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 0.5rem 0.75rem;
            margin-bottom: 0.5rem;
            background: var(--surface-color);
            border: 1px solid var(--border-color);
            border-radius: var(--radius-sm);
        }
        
        .role-item span:first-child {
            font-weight: 500;
            color: var(--text-primary);
        }
        
        .sync-peers-list {
            background: var(--background-color);
            border: 1px solid var(--border-color);
            border-radius: var(--radius-md);
            padding: 1rem;
            margin-top: 1rem;
            max-height: 400px;
            overflow-y: auto;
        }
        
        .sync-peer-item {
            display: flex;
            flex-direction: column;
            gap: 0.5rem;
            padding: 0.75rem;
            background: var(--surface-color);
            border: 1px solid var(--border-color);
            border-radius: var(--radius-sm);
            margin-bottom: 0.5rem;
        }
        
        .sync-peer-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            font-weight: 600;
            color: var(--text-primary);
        }
        
        .sync-peer-info {
            display: flex;
            flex-direction: column;
            gap: 0.25rem;
            font-size: 0.875rem;
            color: var(--text-secondary);
        }
        
        .sync-peer-actions {
            display: flex;
            gap: 0.5rem;
            margin-top: 0.5rem;
        }
        
        .sync-peer-actions .btn {
            padding: 0.375rem 0.75rem;
            font-size: 0.75rem;
        }
        
        .btn-danger {
            background: var(--error-color);
            color: white;
            border-color: var(--error-color);
        }
        
        .btn-danger:hover {
            background: #dc2626;
            border-color: #dc2626;
        }
        
        .btn-full {
            width: 100%;
        }
        
        .status-indicator {
            display: inline-block;
            width: 8px;
            height: 8px;
            border-radius: 50%;
            margin-right: 0.5rem;
        }
        
        .status-indicator.active {
            background: var(--success-color);
        }
        
        .status-indicator.inactive {
            background: var(--error-color);
        }
        
        .input-group {
            display: flex;
            gap: 0.5rem;
            align-items: center;
        }
        
        .input-group input {
            flex: 1;
        }
        
        .input-group .btn {
            flex-shrink: 0;
        }
        
        /* Tool Arguments Panel */
        .tool-args-panel {
            background: var(--surface-color);
            border: 1px solid var(--border-color);
            border-radius: var(--radius-md);
            margin: 0.5rem;
            box-shadow: var(--shadow-sm);
        }
        
        .tool-args-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 0.5rem 0.75rem;
            background: var(--background-color);
            border-bottom: 1px solid var(--border-color);
            border-radius: var(--radius-md) var(--radius-md) 0 0;
            font-weight: 500;
            font-size: 0.875rem;
            color: var(--text-primary);
        }
        
        .tool-args-content {
            padding: 0.75rem;
        }
        
        .tool-args-section h6 {
            margin: 0 0 0.75rem 0;
            font-size: 0.875rem;
            font-weight: 600;
            color: var(--text-primary);
        }
        
        .tool-args-section .form-group {
            margin-bottom: 0.5rem;
        }
        
        .tool-args-section .form-group:last-child {
            margin-bottom: 0;
        }
        
        .tool-args-section label {
            font-size: 0.8rem;
            margin-bottom: 0.25rem;
        }
        
        .tool-args-section input[type="text"],
        .tool-args-section input[type="number"],
        .tool-args-section select {
            font-size: 0.8rem;
            padding: 0.25rem 0.5rem;
        }
        
        .btn-small {
            background: none;
            border: none;
            color: var(--text-secondary);
            cursor: pointer;
            font-size: 0.75rem;
            padding: 0.25rem;
            border-radius: var(--radius-sm);
        }
        
        .btn-small:hover {
            background: var(--border-color);
            color: var(--text-primary);
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <div class="header-left">
                <div class="status" id="status">
                    Connecting to WebSocket...
                </div>
                <h1>Aranya Visualization</h1>
            </div>
            <p>Click to place nodes. Drag to move them. Pan by dragging empty space. Zoom with mouse wheel. Right-click for options.</p>
        </div>
        
        <div class="main-content">
            <div class="panel" id="leftPanel" style="width: 350px; border-right: 1px solid var(--border-color);">
                <div class="drawer" data-drawer-id="appearance">
                    <div class="drawer-header expanded">
                        <div class="drawer-title">
                            <span class="drawer-icon">🎨</span>
                            <span>Appearance</span>
                        </div>
                        <span class="drawer-arrow">▶</span>
                    </div>
                    <div class="drawer-content expanded">
                        <div class="drawer-inner">
                            <div class="texture-selector">
                                <h5>Background Themes</h5>
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
                                    
                                    <div class="texture-pack-management">
                                        <h5>💾 Texture Pack Management</h5>
                                        
                                        <div class="form-group">
                                            <label for="texturePackName">Pack Name:</label>
                                            <input type="text" id="texturePackName" placeholder="My Custom Pack" class="pack-name-input">
                                        </div>
                                        
                                        <div class="pack-actions">
                                            <button id="saveAsTexturePack" class="btn btn-primary">💾 Save As New Pack</button>
                                            <button id="saveCurrentTexturePack" class="btn btn-success">💾 Update Current Pack</button>
                                        </div>
                                        
                                        <div class="saved-packs-section">
                                            <h6>📁 Saved Texture Packs</h6>
                                            <div id="savedTexturePacksList" class="saved-packs-list"></div>
                                        </div>
                                        
                                        <div class="custom-texture-actions">
                                            <button id="resetCustomTexture" class="btn btn-secondary">🔄 Reset to Defaults</button>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
                
                <div class="drawer" data-drawer-id="messaging">
                    <div class="drawer-header expanded">
                        <div class="drawer-title">
                            <span class="drawer-icon">💬</span>
                            <span>Messaging</span>
                        </div>
                        <span class="drawer-arrow">▶</span>
                    </div>
                    <div class="drawer-content expanded">
                        <div class="drawer-inner">
                            <h5>📤 Send Message</h5>
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
                            
                            <h5 style="margin-top: 1.5rem;">📜 Recent Messages</h5>
                            <div id="recentMessages" class="message-history">
                                <p style="color: var(--text-secondary); font-style: italic; text-align: center; margin: 1rem 0;">No messages yet</p>
                            </div>
                        </div>
                    </div>
                </div>
                
                <div class="drawer" data-drawer-id="access-control">
                    <div class="drawer-header">
                        <div class="drawer-title">
                            <span class="drawer-icon">🔐</span>
                            <span>Access Control</span>
                        </div>
                        <span class="drawer-arrow">▶</span>
                    </div>
                    <div class="drawer-content">
                        <div class="drawer-inner">
                            <h5>🔄 Role Management</h5>
                            <div class="form-group">
                                <label for="roleActingNodeSelector">Acting Node (who assigns):</label>
                                <select id="roleActingNodeSelector">
                                    <option value="">-- Select Acting Node --</option>
                                </select>
                            </div>
                            <div class="form-group">
                                <label for="roleTargetNodeSelector">Target Node (who gets role):</label>
                                <select id="roleTargetNodeSelector">
                                    <option value="">-- Select Target Node --</option>
                                </select>
                            </div>
                            <div class="form-group">
                                <label for="roleTeamSelector">Select Team:</label>
                                <select id="roleTeamSelector">
                                    <option value="">-- Select Team --</option>
                                </select>
                            </div>
                            <div class="form-group">
                                <label for="newRoleSelector">Assign Role:</label>
                                <select id="newRoleSelector">
                                    <option value="">-- Select Role --</option>
                                    <option value="Owner">👑 Owner</option>
                                    <option value="Admin">👤 Admin</option>
                                    <option value="Operator">⚡ Operator</option>
                                    <option value="Member">👥 Member</option>
                                </select>
                            </div>
                            <div class="btn-grid">
                                <button onclick="assignRoleFromUI()" class="btn btn-primary" id="assignRoleBtn" disabled>
                                    Assign Role
                                </button>
                                <button onclick="revokeRoleFromUI()" class="btn btn-warning" id="revokeRoleBtn" disabled>
                                    Revoke Role
                                </button>
                            </div>
                            
                            <h5 style="margin-top: 1.5rem;">❌ Device Management</h5>
                            <div class="form-group">
                                <label for="deviceRemovalNodeSelector">Select Device to Remove:</label>
                                <select id="deviceRemovalNodeSelector">
                                    <option value="">-- Select Device --</option>
                                </select>
                            </div>
                            <div class="form-group">
                                <label for="deviceRemovalTeamSelector">From Team:</label>
                                <select id="deviceRemovalTeamSelector">
                                    <option value="">-- Select Team --</option>
                                </select>
                            </div>
                            <button onclick="removeDeviceFromTeamUI()" class="btn btn-danger btn-full" id="removeDeviceBtn" disabled>
                                🗑️ Remove Device from Team
                            </button>
                            
                        </div>
                    </div>
                </div>
                
                <div class="drawer" data-drawer-id="sync-config">
                    <div class="drawer-header">
                        <div class="drawer-title">
                            <span class="drawer-icon">🔄</span>
                            <span>Sync Configuration</span>
                        </div>
                        <span class="drawer-arrow">▶</span>
                    </div>
                    <div class="drawer-content">
                        <div class="drawer-inner">
                            <h5>➕ Add Sync Connection</h5>
                            <div class="form-group">
                                <label for="syncFromNodeSelector">From Node:</label>
                                <select id="syncFromNodeSelector">
                                    <option value="">-- Select Source Node --</option>
                                </select>
                            </div>
                            <div class="form-group">
                                <label for="syncToNodeSelector">To Node:</label>
                                <select id="syncToNodeSelector">
                                    <option value="">-- Select Target Node --</option>
                                </select>
                            </div>
                            <div class="form-group">
                                <label for="syncTeamSelector">Team:</label>
                                <select id="syncTeamSelector">
                                    <option value="">-- Select Team --</option>
                                </select>
                            </div>
                            <div class="form-group">
                                <label for="syncInterval">Sync Interval (seconds):</label>
                                <input type="number" id="syncInterval" min="1" value="5" placeholder="5">
                            </div>
                            <div class="form-group">
                                <label>
                                    <input type="checkbox" id="syncNowFlag" checked>
                                    Sync immediately after adding
                                </label>
                            </div>
                            
                            <div class="form-group">
                                <label>
                                    <input type="checkbox" id="advancedMode" onchange="toggleAdvancedMode()">
                                    Advanced mode (custom URL)
                                </label>
                            </div>
                            
                            <div id="advancedSyncOptions" style="display: none;">
                                <div class="form-group">
                                    <label for="customSyncUrl">Custom Peer URL:</label>
                                    <input type="text" id="customSyncUrl" placeholder="http://192.168.1.100:8080">
                                    <small style="display: block; color: var(--text-secondary); margin-top: 0.25rem;">
                                        Use this to sync with nodes outside the visualization
                                    </small>
                                </div>
                            </div>
                            
                            <button onclick="addSyncConnectionFromUI()" class="btn btn-primary btn-full" id="addSyncBtn" disabled>
                                ➕ Add Sync Connection
                            </button>
                        </div>
                    </div>
                </div>
            </div>
            
            <div class="resizer" id="leftResizer"></div>
            
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
                
                <!-- Tool Arguments Panel -->
                <div id="toolArgsPanel" class="tool-args-panel" style="display: none;">
                    <div class="tool-args-header">
                        <span id="toolArgsTitle">Tool Arguments</span>
                        <button onclick="toggleToolArgs()" class="btn-small">▲</button>
                    </div>
                    <div id="toolArgsContent" class="tool-args-content">
                        <!-- Create Node Arguments -->
                        <div id="createNodeArgs" class="tool-args-section" style="display: none;">
                            <h6>Create Node Options</h6>
                            <div class="form-group">
                                <label for="createNodeName">Node Name:</label>
                                <input type="text" id="createNodeName" placeholder="Auto-generated if empty">
                            </div>
                            <div class="form-group">
                                <label for="createNodeIcon">Node Icon:</label>
                                <input type="text" id="createNodeIcon" placeholder="🔵" maxlength="2">
                            </div>
                            <div class="form-group">
                                <label for="createNodeTeam">Join Team (optional):</label>
                                <select id="createNodeTeam">
                                    <option value="">-- No team --</option>
                                </select>
                            </div>
                        </div>
                        
                        <!-- Connect Nodes Arguments -->
                        <div id="connectNodesArgs" class="tool-args-section" style="display: none;">
                            <h6>Connection Options</h6>
                            <div class="form-group">
                                <label for="connectTeam">Team:</label>
                                <select id="connectTeam">
                                    <option value="">-- Select Team --</option>
                                </select>
                            </div>
                            <div class="form-group">
                                <label for="connectInterval">Sync Interval (seconds):</label>
                                <input type="number" id="connectInterval" min="1" max="3600" value="5">
                            </div>
                            <div class="form-group">
                                <label>
                                    <input type="checkbox" id="connectBidirectional" checked>
                                    Bidirectional sync
                                </label>
                            </div>
                            <div class="form-group">
                                <label>
                                    <input type="checkbox" id="connectRandomInterval">
                                    Random interval (1-10s)
                                </label>
                            </div>
                            <div class="form-group">
                                <label>
                                    <input type="checkbox" id="connectSyncNow" checked>
                                    Sync immediately
                                </label>
                            </div>
                        </div>
                    </div>
                </div>
                
                <div class="canvas-viewport">
                    <canvas id="canvas"></canvas>
                </div>
            </div>
            
            <div class="resizer" id="rightResizer"></div>
            
            <div class="panel" id="rightPanel" style="width: 320px; border-left: 1px solid var(--border-color);">
                <div class="drawer" data-drawer-id="team-creation">
                    <div class="drawer-header expanded">
                        <div class="drawer-title">
                            <span class="drawer-icon">🏢</span>
                            <span>Team Creation</span>
                        </div>
                        <span class="drawer-arrow">▶</span>
                    </div>
                    <div class="drawer-content expanded">
                        <div class="drawer-inner">
                            <h5>Create Team</h5>
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
                            
                            <h5 style="margin-top: 1.5rem;">Join Team</h5>
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
                        </div>
                    </div>
                </div>
                
                <div class="drawer" data-drawer-id="team-operations">
                    <div class="drawer-header expanded">
                        <div class="drawer-title">
                            <span class="drawer-icon">⚙️</span>
                            <span>Team Operations</span>
                        </div>
                        <span class="drawer-arrow">▶</span>
                    </div>
                    <div class="drawer-content expanded">
                        <div class="drawer-inner">
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
                            
                            <div class="role-legend" style="margin-top: 1rem;">
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
                        </div>
                    </div>
                </div>
                
                <div class="drawer" data-drawer-id="team-overview">
                    <div class="drawer-header expanded">
                        <div class="drawer-title">
                            <span class="drawer-icon">📊</span>
                            <span>Team Overview</span>
                        </div>
                        <span class="drawer-arrow">▶</span>
                    </div>
                    <div class="drawer-content expanded">
                        <div class="drawer-inner">
                            <h5>All Teams</h5>
                            <div id="teamsList"></div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
        
        <div class="notification-area" id="notificationArea">
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
        let currentCustomPackId = null; // Track which saved pack is currently loaded
        
        // Image cache for node icons
        const nodeImageCache = new Map(); // imageData -> HTMLImageElement
        
        // Saved texture packs storage
        const savedTexturePacks = new Map(); // packId -> pack data
        
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
            
            // Texture pack management buttons
            document.getElementById('saveAsTexturePack').addEventListener('click', saveAsNewTexturePack);
            document.getElementById('saveCurrentTexturePack').addEventListener('click', updateCurrentTexturePack);
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
            // Legacy function - now redirects to new system
            // Save to localStorage for backward compatibility
            const customPack = {
                backgroundImage: texturePacks.custom.backgroundImage,
                backgroundOpacity: texturePacks.custom.backgroundOpacity,
                backgroundTheme: texturePacks.custom.backgroundTheme,
                nodeIcons: texturePacks.custom.nodeIcons,
                customNodeIcons: Array.from(texturePacks.custom.customNodeIcons.entries()),
                customNodeImages: Array.from(texturePacks.custom.customNodeImages.entries())
            };
            
            localStorage.setItem('aranya-custom-texture-pack', JSON.stringify(customPack));
            
            // Also save preferences
            savePreferences();
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
        
        // Texture Pack Management Functions
        function saveAsNewTexturePack() {
            const packName = document.getElementById('texturePackName').value.trim();
            if (!packName) {
                showNotification('Please enter a name for your texture pack', 'warning');
                return;
            }
            
            const packId = 'pack_' + Date.now() + '_' + Math.random().toString(36).substr(2, 9);
            const packData = {
                id: packId,
                name: packName,
                created: new Date().toISOString(),
                backgroundImage: texturePacks.custom.backgroundImage,
                backgroundOpacity: texturePacks.custom.backgroundOpacity,
                backgroundTheme: texturePacks.custom.backgroundTheme,
                nodeIcons: texturePacks.custom.nodeIcons,
                customNodeIcons: Array.from(texturePacks.custom.customNodeIcons.entries()),
                customNodeImages: Array.from(texturePacks.custom.customNodeImages.entries())
            };
            
            savedTexturePacks.set(packId, packData);
            currentCustomPackId = packId;
            saveAllTexturePacks();
            updateSavedPacksList();
            
            // Clear pack name input
            document.getElementById('texturePackName').value = '';
            
            // Show success message
            showTexturePackMessage('Texture pack saved successfully!', 'success');
        }
        
        function updateCurrentTexturePack() {
            if (!currentCustomPackId) {
                showNotification('No texture pack is currently loaded. Use "Save As New Pack" instead.', 'warning');
                return;
            }
            
            const packData = savedTexturePacks.get(currentCustomPackId);
            if (!packData) {
                showNotification('Current texture pack not found. Use "Save As New Pack" instead.', 'warning');
                return;
            }
            
            // Update the existing pack
            packData.backgroundImage = texturePacks.custom.backgroundImage;
            packData.backgroundOpacity = texturePacks.custom.backgroundOpacity;
            packData.backgroundTheme = texturePacks.custom.backgroundTheme;
            packData.nodeIcons = texturePacks.custom.nodeIcons;
            packData.customNodeIcons = Array.from(texturePacks.custom.customNodeIcons.entries());
            packData.customNodeImages = Array.from(texturePacks.custom.customNodeImages.entries());
            packData.modified = new Date().toISOString();
            
            savedTexturePacks.set(currentCustomPackId, packData);
            saveAllTexturePacks();
            updateSavedPacksList();
            
            showTexturePackMessage('Texture pack updated successfully!', 'success');
        }
        
        function loadTexturePack(packId) {
            const packData = savedTexturePacks.get(packId);
            if (!packData) {
                showNotification('Texture pack not found', 'error');
                return;
            }
            
            // Load pack data into current custom texture pack
            texturePacks.custom.backgroundImage = packData.backgroundImage;
            texturePacks.custom.backgroundOpacity = packData.backgroundOpacity || 80;
            texturePacks.custom.backgroundTheme = packData.backgroundTheme || 'light';
            texturePacks.custom.nodeIcons = packData.nodeIcons || '🔵';
            texturePacks.custom.customNodeIcons = new Map(packData.customNodeIcons || []);
            texturePacks.custom.customNodeImages = new Map(packData.customNodeImages || []);
            
            currentCustomPackId = packId;
            customBackgroundImage = packData.backgroundImage;
            
            // Update UI
            document.getElementById('backgroundTheme').value = texturePacks.custom.backgroundTheme;
            document.getElementById('backgroundOpacity').value = texturePacks.custom.backgroundOpacity;
            document.getElementById('opacityValue').textContent = texturePacks.custom.backgroundOpacity + '%';
            document.getElementById('defaultNodeIcon').value = texturePacks.custom.nodeIcons;
            document.getElementById('texturePackName').value = packData.name;
            
            // Switch to custom texture pack and apply
            setTexturePack('custom');
            updateIndividualNodeIcons();
            updateSavedPacksList();
            
            showTexturePackMessage(`Loaded "${packData.name}" texture pack`, 'success');
        }
        
        function deleteTexturePack(packId) {
            const packData = savedTexturePacks.get(packId);
            if (!packData) return;
            
            if (confirm(`Are you sure you want to delete "${packData.name}"?`)) {
                savedTexturePacks.delete(packId);
                
                // If this was the current pack, clear the reference
                if (currentCustomPackId === packId) {
                    currentCustomPackId = null;
                    document.getElementById('texturePackName').value = '';
                }
                
                saveAllTexturePacks();
                updateSavedPacksList();
                showTexturePackMessage('Texture pack deleted', 'success');
            }
        }
        
        function saveAllTexturePacks() {
            const packsArray = Array.from(savedTexturePacks.entries()).map(([id, data]) => ({
                id: id,
                ...data
            }));
            localStorage.setItem('aranya-saved-texture-packs', JSON.stringify(packsArray));
        }
        
        function loadAllTexturePacks() {
            const saved = localStorage.getItem('aranya-saved-texture-packs');
            if (saved) {
                try {
                    const packsArray = JSON.parse(saved);
                    savedTexturePacks.clear();
                    packsArray.forEach(pack => {
                        savedTexturePacks.set(pack.id, pack);
                    });
                    updateSavedPacksList();
                } catch (e) {
                    console.warn('Failed to load saved texture packs:', e);
                }
            }
        }
        
        function updateSavedPacksList() {
            const container = document.getElementById('savedTexturePacksList');
            container.innerHTML = '';
            
            if (savedTexturePacks.size === 0) {
                container.innerHTML = '<div style="padding: 1rem; text-align: center; color: var(--text-secondary); font-style: italic; font-size: 0.875rem;">No saved texture packs</div>';
                return;
            }
            
            const sortedPacks = Array.from(savedTexturePacks.values()).sort((a, b) => 
                new Date(b.created) - new Date(a.created)
            );
            
            sortedPacks.forEach(pack => {
                const item = document.createElement('div');
                item.className = 'saved-pack-item';
                if (currentCustomPackId === pack.id) {
                    item.classList.add('active');
                }
                
                const packInfo = document.createElement('div');
                packInfo.className = 'pack-info';
                
                const packName = document.createElement('div');
                packName.className = 'pack-name';
                packName.textContent = pack.name;
                
                const packDetails = document.createElement('div');
                packDetails.className = 'pack-details';
                const createdDate = new Date(pack.created).toLocaleDateString();
                const nodeCount = pack.customNodeImages ? pack.customNodeImages.length : 0;
                const hasBackground = pack.backgroundImage ? 'Background' : 'No background';
                packDetails.textContent = `${createdDate} • ${nodeCount} custom icons • ${hasBackground}`;
                
                packInfo.appendChild(packName);
                packInfo.appendChild(packDetails);
                
                const actionsContainer = document.createElement('div');
                actionsContainer.className = 'pack-actions-buttons';
                
                const loadBtn = document.createElement('button');
                loadBtn.className = 'pack-action-btn load';
                loadBtn.textContent = 'Load';
                loadBtn.addEventListener('click', () => loadTexturePack(pack.id));
                
                const deleteBtn = document.createElement('button');
                deleteBtn.className = 'pack-action-btn delete';
                deleteBtn.textContent = '×';
                deleteBtn.title = 'Delete pack';
                deleteBtn.addEventListener('click', () => deleteTexturePack(pack.id));
                
                actionsContainer.appendChild(loadBtn);
                actionsContainer.appendChild(deleteBtn);
                
                item.appendChild(packInfo);
                item.appendChild(actionsContainer);
                container.appendChild(item);
            });
        }
        
        function showTexturePackMessage(message, type = 'info') {
            // Create a temporary message element
            const messageEl = document.createElement('div');
            messageEl.style.position = 'fixed';
            messageEl.style.top = '20px';
            messageEl.style.right = '20px';
            messageEl.style.padding = '0.75rem 1rem';
            messageEl.style.borderRadius = 'var(--radius-md)';
            messageEl.style.color = 'white';
            messageEl.style.fontSize = '0.875rem';
            messageEl.style.zIndex = '10000';
            messageEl.style.boxShadow = 'var(--shadow-lg)';
            messageEl.textContent = message;
            
            if (type === 'success') {
                messageEl.style.background = 'var(--success-color)';
            } else if (type === 'error') {
                messageEl.style.background = 'var(--error-color)';
            } else {
                messageEl.style.background = 'var(--primary-color)';
            }
            
            document.body.appendChild(messageEl);
            
            // Remove after 3 seconds
            setTimeout(() => {
                if (messageEl.parentNode) {
                    messageEl.parentNode.removeChild(messageEl);
                }
            }, 3000);
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
            currentCustomPackId = null;
            
            // Reset UI
            document.getElementById('backgroundTheme').value = 'light';
            document.getElementById('backgroundUpload').value = '';
            document.getElementById('backgroundOpacity').value = 80;
            document.getElementById('opacityValue').textContent = '80%';
            document.getElementById('defaultNodeIcon').value = '🔵';
            document.getElementById('texturePackName').value = '';
            
            // Clear localStorage
            localStorage.removeItem('aranya-custom-texture-pack');
            
            updateIndividualNodeIcons();
            updateSavedPacksList();
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
        // Resizable panels functionality
        function initializeResizers() {
            const leftPanel = document.getElementById('leftPanel');
            const rightPanel = document.getElementById('rightPanel');
            const leftResizer = document.getElementById('leftResizer');
            const rightResizer = document.getElementById('rightResizer');
            const container = document.querySelector('.main-content');
            
            let isResizing = false;
            let currentResizer = null;
            let startX = 0;
            let startWidth = 0;
            let targetPanel = null;
            
            function startResize(e, resizer, panel) {
                isResizing = true;
                currentResizer = resizer;
                targetPanel = panel;
                startX = e.clientX;
                startWidth = parseInt(window.getComputedStyle(panel).width, 10);
                
                document.body.style.cursor = 'col-resize';
                document.body.style.userSelect = 'none';
                
                document.addEventListener('mousemove', handleMouseMove);
                document.addEventListener('mouseup', stopResize);
            }
            
            function handleMouseMove(e) {
                if (!isResizing) return;
                
                const diff = e.clientX - startX;
                let newWidth = startWidth;
                
                if (currentResizer === leftResizer) {
                    newWidth = startWidth + diff;
                } else {
                    newWidth = startWidth - diff;
                }
                
                // Enforce min/max constraints
                newWidth = Math.max(280, Math.min(600, newWidth));
                targetPanel.style.width = newWidth + 'px';
                
                // Trigger canvas redraw
                if (typeof draw !== 'undefined') {
                    draw();
                }
            }
            
            function stopResize() {
                isResizing = false;
                currentResizer = null;
                targetPanel = null;
                
                document.body.style.cursor = '';
                document.body.style.userSelect = '';
                
                document.removeEventListener('mousemove', handleMouseMove);
                document.removeEventListener('mouseup', stopResize);
                
                // Save panel widths to localStorage
                localStorage.setItem('leftPanelWidth', leftPanel.style.width);
                localStorage.setItem('rightPanelWidth', rightPanel.style.width);
            }
            
            // Add event listeners
            leftResizer.addEventListener('mousedown', (e) => startResize(e, leftResizer, leftPanel));
            rightResizer.addEventListener('mousedown', (e) => startResize(e, rightResizer, rightPanel));
            
            // Restore saved widths
            const savedLeftWidth = localStorage.getItem('leftPanelWidth');
            const savedRightWidth = localStorage.getItem('rightPanelWidth');
            
            if (savedLeftWidth) {
                leftPanel.style.width = savedLeftWidth;
            }
            if (savedRightWidth) {
                rightPanel.style.width = savedRightWidth;
            }
        }
        
        // Drawer functionality
        function initializeDrawers() {
            const drawers = document.querySelectorAll('.drawer');
            
            drawers.forEach(drawer => {
                const header = drawer.querySelector('.drawer-header');
                const content = drawer.querySelector('.drawer-content');
                
                // Load saved state from localStorage
                const drawerId = drawer.dataset.drawerId;
                const savedState = localStorage.getItem(`drawer-${drawerId}`);
                if (savedState === 'expanded' || savedState === null) {
                    header.classList.add('expanded');
                    content.classList.add('expanded');
                    
                    // Update selectors for initially expanded drawers
                    if (drawerId === 'access-control') {
                        setTimeout(() => updateAccessControlSelectors(), 100);
                    } else if (drawerId === 'sync-config') {
                        setTimeout(() => updateSyncConfigSelectors(), 100);
                    }
                }
                
                header.addEventListener('click', () => {
                    const isExpanded = header.classList.contains('expanded');
                    
                    if (isExpanded) {
                        header.classList.remove('expanded');
                        content.classList.remove('expanded');
                        localStorage.setItem(`drawer-${drawerId}`, 'collapsed');
                    } else {
                        header.classList.add('expanded');
                        content.classList.add('expanded');
                        localStorage.setItem(`drawer-${drawerId}`, 'expanded');
                        
                        // Update selectors when access control or sync config drawers are opened
                        if (drawerId === 'access-control') {
                            updateAccessControlSelectors();
                        } else if (drawerId === 'sync-config') {
                            updateSyncConfigSelectors();
                        }
                    }
                });
            });
        }
        
        let ws = null;
        let nodes = new Map();
        let connections = new Map();
        let teams = new Map();
        let canvas = null;
        let ctx = null;
        
        // Initialize canvas size
        function initializeCanvas() {
            // Get canvas if not already initialized
            if (!canvas) {
                canvas = document.getElementById('canvas');
                if (!canvas) {
                    console.error('Canvas element not found');
                    return;
                }
                ctx = canvas.getContext('2d');
            }
            
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
                    // Only update selectors if the access control or sync config drawers are open
                    if (document.querySelector('[data-drawer-id="access-control"] .drawer-content.expanded')) {
                        updateAccessControlSelectors();
                    }
                    if (document.querySelector('[data-drawer-id="sync-config"] .drawer-content.expanded')) {
                        updateSyncConfigSelectors();
                    }
                    draw();
                    break;
                case 'NodeCreated':
                    nodes.set(message.node.id, message.node);
                    updateNodeSelectors();
                    
                    // Handle pending node configuration from tool args
                    if (window.pendingNodeConfig) {
                        const config = window.pendingNodeConfig;
                        const nodeId = message.node.id;
                        
                        // Set custom icon if specified
                        if (config.icon && currentTexturePack === 'custom') {
                            texturePacks.custom.customNodeIcons.set(nodeId, config.icon);
                        }
                        
                        // Join team if specified
                        if (config.teamId && teams.has(config.teamId)) {
                            const team = teams.get(config.teamId);
                            setTimeout(() => {
                                ws.send(JSON.stringify({
                                    type: 'JoinTeam',
                                    node_id: nodeId,
                                    team_id: config.teamId,
                                    owner_node_id: team.owner_node_id
                                }));
                            }, 500); // Small delay to ensure node is fully created
                        }
                        
                        // Clear pending config
                        delete window.pendingNodeConfig;
                    }
                    
                    // Only update selectors if the drawers are open
                    if (document.querySelector('[data-drawer-id="access-control"] .drawer-content.expanded')) {
                        updateAccessControlSelectors();
                    }
                    if (document.querySelector('[data-drawer-id="sync-config"] .drawer-content.expanded')) {
                        updateSyncConfigSelectors();
                    }
                    // Update tool args selectors
                    updateToolArgsSelectors();
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
                        // Only update selectors if the drawers are open
                        if (document.querySelector('[data-drawer-id="access-control"] .drawer-content.expanded')) {
                            updateAccessControlSelectors();
                        }
                        if (document.querySelector('[data-drawer-id="sync-config"] .drawer-content.expanded')) {
                            updateSyncConfigSelectors();
                        }
                        draw();
                        
                        // Check if this is part of a quick setup
                        if (window.pendingQuickSetup) {
                            completeQuickSetup(team.id);
                        } else {
                            showNotification(`Team "${team.name}" created successfully!`, 'success');
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
                        
                        // Only show notification if not part of bulk operations
                        if (!window.pendingQuickSetup) {
                            showNotification(`${joinedNode.name} joined team "${message.team.name}"`, 'success');
                        }
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
                case 'RoleAssigned':
                    console.log('Received RoleAssigned message:', message);
                    showNotification(`Role ${message.role} assigned to device successfully`, 'success');
                    // Update the target node with role information
                    const targetNode = nodes.get(message.target_node_id);
                    console.log('Target node found:', targetNode);
                    if (targetNode && targetNode.teams) {
                        // Find the team in the node's teams array and update the role
                        const teamIndex = targetNode.teams.findIndex(t => t.id === message.team_id);
                        console.log('Team index found:', teamIndex, 'for team ID:', message.team_id);
                        if (teamIndex !== -1) {
                            console.log('Updating role from', targetNode.teams[teamIndex].role, 'to', message.role);
                            targetNode.teams[teamIndex].role = message.role;
                        }
                    }
                    draw();
                    break;
                case 'RoleRevoked':
                    console.log('Received RoleRevoked message:', message);
                    showNotification(`Role revoked from device successfully (demoted to Member)`, 'success');
                    // Update the target node
                    const revokedTargetNode = nodes.get(message.target_node_id);
                    console.log('Revoked target node found:', revokedTargetNode);
                    if (revokedTargetNode && revokedTargetNode.teams) {
                        // Find the team in the node's teams array and update the role
                        const teamIndex = revokedTargetNode.teams.findIndex(t => t.id === message.team_id);
                        console.log('Team index found for revocation:', teamIndex, 'for team ID:', message.team_id);
                        if (teamIndex !== -1) {
                            console.log('Updating role from', revokedTargetNode.teams[teamIndex].role, 'to Member');
                            revokedTargetNode.teams[teamIndex].role = 'Member'; // Revocation always demotes to Member
                        }
                    }
                    draw();
                    break;
                case 'TeamRolesResponse':
                    displayTeamRoles(message.team_id, message.devices);
                    break;
                case 'SyncPeersResponse':
                    displaySyncPeers(message.team_id, message.peers);
                    break;
                case 'DeviceRemovedFromTeam':
                    console.log('Received DeviceRemovedFromTeam message:', message);
                    showNotification('Device removed from team successfully', 'success');
                    // Update the node by removing the team
                    const removedNode = nodes.get(message.node_id);
                    console.log('Updating node:', removedNode);
                    if (removedNode && removedNode.teams) {
                        const oldTeamsCount = removedNode.teams.length;
                        removedNode.teams = removedNode.teams.filter(t => t.id !== message.team_id);
                        console.log('Teams updated from', oldTeamsCount, 'to', removedNode.teams.length);
                    }
                    // Update UI
                    updateTeamsUI();
                    updateAccessControlSelectors();
                    draw();
                    break;
                case 'Error':
                    console.error('WebSocket error message:', message);
                    showNotification(message.message, 'error');
                    break;
                default:
                    console.log('Unhandled WebSocket message type:', message.type, message);
                    break;
            }
        }

        // Display functions for responses
        function displayTeamRoles(teamId, devices) {
            const display = document.getElementById('teamRolesList');
            display.innerHTML = '';
            
            if (!devices || devices.length === 0) {
                display.innerHTML = '<p style="color: var(--text-secondary); font-style: italic;">No devices in this team</p>';
                return;
            }
            
            devices.forEach(device => {
                const roleItem = document.createElement('div');
                roleItem.className = 'role-item';
                
                const deviceName = getNodeName(device.device_id) || device.device_id.substring(0, 8) + '...';
                const roleBadge = document.createElement('span');
                roleBadge.className = `role-badge ${device.role.toLowerCase()}`;
                roleBadge.textContent = device.role;
                
                roleItem.innerHTML = `
                    <span>${deviceName}</span>
                `;
                roleItem.appendChild(roleBadge);
                display.appendChild(roleItem);
            });
        }
        
        function getNodeName(nodeId) {
            const node = nodes.get(nodeId);
            return node ? node.name : null;
        }
        
        function displaySyncPeers(teamId, peers) {
            const display = document.getElementById('syncPeersDisplay');
            display.innerHTML = '';
            
            if (!peers || peers.length === 0) {
                display.innerHTML = '<p style="color: var(--text-secondary); font-style: italic;">No sync peers configured for this team</p>';
                return;
            }
            
            peers.forEach(peer => {
                const peerItem = document.createElement('div');
                peerItem.className = 'sync-peer-item';
                
                peerItem.innerHTML = `
                    <div class="sync-peer-header">
                        <span>${peer.address}</span>
                        <span class="status-indicator ${peer.active ? 'active' : 'inactive'}"></span>
                    </div>
                    <div class="sync-peer-info">
                        <span>Interval: ${peer.interval_secs} seconds</span>
                        <span>Sync on add: ${peer.sync_now ? 'Yes' : 'No'}</span>
                    </div>
                    <div class="sync-peer-actions">
                        <div class="input-group">
                            <input type="number" value="${peer.interval_secs}" min="1" placeholder="Interval" id="interval-${peer.address.replace(/[:.]/g, '_')}">
                            <button class="btn btn-secondary" onclick="updateSyncPeerConfig('${peer.address}', '${teamId}', document.getElementById('interval-${peer.address.replace(/[:.]/g, '_')}').value)">
                                Update
                            </button>
                        </div>
                        <button class="btn btn-danger" onclick="removeSyncPeer('${peer.address}', '${teamId}')">
                            Remove
                        </button>
                    </div>
                `;
                display.appendChild(peerItem);
            });
        }

        // Notification system
        let suppressNotifications = false;
        
        function showNotification(message, type = 'info', title = null) {
            // Skip if notifications are suppressed
            if (suppressNotifications) return;
            
            const notificationArea = document.getElementById('notificationArea');
            const notification = document.createElement('div');
            notification.className = `notification ${type}`;
            
            const icons = {
                success: '✅',
                warning: '⚠️',
                error: '❌',
                info: 'ℹ️'
            };
            
            const titles = {
                success: 'Success',
                warning: 'Warning', 
                error: 'Error',
                info: 'Info'
            };
            
            notification.innerHTML = `
                <div class="notification-icon">${icons[type]}</div>
                <div class="notification-content">
                    <div class="notification-title">${title || titles[type]}</div>
                    <div class="notification-message">${message}</div>
                </div>
            `;
            
            notificationArea.appendChild(notification);
            
            // Auto-remove after 5 seconds
            setTimeout(() => {
                notification.style.animation = 'fadeOut 0.3s ease-out';
                setTimeout(() => {
                    notification.remove();
                }, 300);
            }, 5000);
        }

        function draw() {
            if (!ctx || !canvas) return;
            
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
            
            // Update message bubble positions after drawing
            updateAllMessageBubbles();
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

        // Add all canvas event listeners inside a setup function
        function setupCanvasEventListeners() {
            if (!canvas) return;
            
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
                    
                    // Get node name from tool args or fallback to main input
                    const toolArgsName = document.getElementById('createNodeName').value.trim();
                    const nameInput = document.getElementById('nodeNameInput');
                    const fallbackName = nameInput.value.trim() || `Node${nodes.size + 1}`;
                    const name = toolArgsName || fallbackName;
                    
                    // Get other tool args
                    const nodeIcon = document.getElementById('createNodeIcon').value.trim();
                    const joinTeamId = document.getElementById('createNodeTeam').value;
                    
                    // Clear inputs
                    nameInput.value = '';
                    document.getElementById('createNodeName').value = '';
                    
                    // Create the node
                    ws.send(JSON.stringify({
                        type: 'CreateNode',
                        name: name,
                        position: { x: clampedX, y: clampedY }
                    }));
                    
                    // Store icon and team info for when the node is created
                    if (nodeIcon || joinTeamId) {
                        window.pendingNodeConfig = {
                            icon: nodeIcon,
                            teamId: joinTeamId,
                            position: { x: clampedX, y: clampedY }
                        };
                    }
                }
            } else if (currentTool === 'connect') {
                if (nodeId) {
                    if (!connectStart) {
                        // Start connection
                        connectStart = nodeId;
                    } else if (connectStart !== nodeId) {
                        // Complete connection using tool arguments
                        const connectTeamId = document.getElementById('connectTeam').value;
                        let interval = parseInt(document.getElementById('connectInterval').value) || 5;
                        const bidirectional = document.getElementById('connectBidirectional').checked;
                        const randomInterval = document.getElementById('connectRandomInterval').checked;
                        const syncNow = document.getElementById('connectSyncNow').checked;
                        
                        // Use tool args team or fallback to selected team
                        const teamId = connectTeamId || selectedTeamId;
                        if (!teamId) {
                            showNotification('Please select a team in the connection options or main team selector!', 'warning');
                            return;
                        }
                        
                        // Apply random interval if selected
                        if (randomInterval) {
                            interval = Math.floor(Math.random() * 10) + 1; // 1-10 seconds
                        }
                        
                        // Create primary connection
                        ws.send(JSON.stringify({
                            type: 'AddSyncConnection',
                            from: connectStart,
                            to: nodeId,
                            team_id: teamId,
                            interval_secs: interval,
                            sync_now: syncNow
                        }));
                        
                        // Create reverse connection if bidirectional
                        if (bidirectional) {
                            const reverseInterval = randomInterval ? Math.floor(Math.random() * 10) + 1 : interval;
                            ws.send(JSON.stringify({
                                type: 'AddSyncConnection',
                                from: nodeId,
                                to: connectStart,
                                team_id: teamId,
                                interval_secs: reverseInterval,
                                sync_now: syncNow
                            }));
                        }
                        
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
        } // End of setupCanvasEventListeners

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
                    
                    const info = `<strong>${node.name}</strong><br>
Status: ${JSON.stringify(node.status)}<br>
Daemon Port: ${node.daemon_port}<br>
REST Port: ${node.rest_port}<br>
Connections: ${incomingCount} in, ${outgoingCount} out`;
                    showNotification(info, 'info', 'Node Information');
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
            
            // Show/hide tool arguments panel
            showToolArgs(tool);
            
            // Update instructions
            const header = document.querySelector('.header p');
            if (tool === 'select') {
                header.textContent = 'Click and drag to move nodes. Right-click for options.';
            } else if (tool === 'place') {
                header.textContent = 'Click on empty space to place a new node. Configure options below.';
            } else if (tool === 'connect') {
                header.textContent = 'Click on receiver node, then click on source node. Configure connection options below.';
            }
            
            draw();
        }
        
        // Tool Arguments Functions
        function showToolArgs(tool) {
            const panel = document.getElementById('toolArgsPanel');
            const sections = document.querySelectorAll('.tool-args-section');
            
            // Hide all sections first
            sections.forEach(section => section.style.display = 'none');
            
            if (tool === 'place') {
                document.getElementById('createNodeArgs').style.display = 'block';
                document.getElementById('toolArgsTitle').textContent = 'Create Node Options';
                panel.style.display = 'block';
                updateToolArgsSelectors();
            } else if (tool === 'connect') {
                document.getElementById('connectNodesArgs').style.display = 'block';
                document.getElementById('toolArgsTitle').textContent = 'Connection Options';
                panel.style.display = 'block';
                updateToolArgsSelectors();
            } else {
                panel.style.display = 'none';
            }
        }
        
        function toggleToolArgs() {
            const content = document.getElementById('toolArgsContent');
            const button = document.querySelector('.tool-args-header button');
            
            if (content.style.display === 'none') {
                content.style.display = 'block';
                button.textContent = '▲';
            } else {
                content.style.display = 'none';
                button.textContent = '▼';
            }
        }
        
        function updateToolArgsSelectors() {
            // Update create node team selector
            const createNodeTeamSelector = document.getElementById('createNodeTeam');
            if (createNodeTeamSelector) {
                const currentValue = createNodeTeamSelector.value;
                createNodeTeamSelector.innerHTML = '<option value="">-- No team --</option>';
                teams.forEach((team, id) => {
                    const option = document.createElement('option');
                    option.value = id;
                    option.textContent = team.name || `Team ${id.slice(0, 8)}`;
                    createNodeTeamSelector.appendChild(option);
                });
                if (currentValue && teams.has(currentValue)) {
                    createNodeTeamSelector.value = currentValue;
                }
            }
            
            // Update connect team selector
            const connectTeamSelector = document.getElementById('connectTeam');
            if (connectTeamSelector) {
                const currentValue = connectTeamSelector.value;
                connectTeamSelector.innerHTML = '<option value="">-- Select Team --</option>';
                teams.forEach((team, id) => {
                    const option = document.createElement('option');
                    option.value = id;
                    option.textContent = team.name || `Team ${id.slice(0, 8)}`;
                    connectTeamSelector.appendChild(option);
                });
                if (currentValue && teams.has(currentValue)) {
                    connectTeamSelector.value = currentValue;
                }
            }
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
                    showNotification('No teams available to join. Create a team first.', 'info');
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
                        showNotification('Team not found!', 'error');
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
                showNotification('Please select a node to create the team.', 'warning');
                return;
            }
            
            if (!teamName) {
                showNotification('Please enter a team name.', 'warning');
                return;
            }
            
            ws.send(JSON.stringify({
                type: 'CreateTeam',
                node_id: nodeId,
                team_name: teamName
            }));
            
            showNotification(`Creating team "${teamName}"...`, 'info');
            // Clear the input
            teamNameInput.value = '';
        }

        function joinTeamFromUI() {
            const joinNodeSelector = document.getElementById('joinNodeSelector');
            const availableTeamsSelector = document.getElementById('availableTeamsSelector');
            
            const nodeId = joinNodeSelector.value;
            const teamId = availableTeamsSelector.value;
            
            if (!nodeId) {
                showNotification('Please select a node to join the team.', 'warning');
                return;
            }
            
            if (!teamId) {
                showNotification('Please select a team to join.', 'warning');
                return;
            }
            
            const team = teams.get(teamId);
            if (!team) {
                showNotification('Selected team not found.', 'error');
                return;
            }
            
            ws.send(JSON.stringify({
                type: 'JoinTeam',
                node_id: nodeId,
                team_id: teamId,
                owner_node_id: team.owner_node_id
            }));
            
            const node = nodes.get(nodeId);
            showNotification(`Adding ${node ? node.name : 'node'} to team "${team.name}"...`, 'info');
        }

        function addAllNodesToSelectedTeam() {
            if (!selectedTeamId) {
                showNotification('Please select a team first from the dropdown.', 'warning');
                return;
            }

            const team = teams.get(selectedTeamId);
            if (!team) {
                showNotification('Selected team not found.', 'error');
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
                showNotification('All running nodes are already in this team.', 'info');
                return;
            }

            if (confirm(`Add ${nodesToAdd.length} nodes to team "${team.name}"?`)) {
                // Suppress notifications during bulk operation
                suppressNotifications = true;
                
                nodesToAdd.forEach(node => {
                    ws.send(JSON.stringify({
                        type: 'JoinTeam',
                        node_id: node.id,
                        team_id: selectedTeamId,
                        owner_node_id: team.owner_node_id
                    }));
                });
                
                // Show single notification after operations are sent
                setTimeout(() => {
                    suppressNotifications = false;
                    showNotification(`Adding ${nodesToAdd.length} nodes to team "${team.name}"`, 'success');
                }, 100);
            }
        }

        function autoSyncTeamMembers() {
            if (!selectedTeamId) {
                showNotification('Please select a team first from the dropdown.', 'warning');
                return;
            }

            const team = teams.get(selectedTeamId);
            if (!team) {
                showNotification('Selected team not found.', 'error');
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
                showNotification('Need at least 2 nodes in the team to create sync connections.', 'warning');
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
                showNotification('All team members are already fully synced.', 'info');
                return;
            }

            if (confirm(`Create ${connectionCount} sync connections between all team members?`)) {
                // Suppress notifications during bulk operation
                suppressNotifications = true;
                
                connectionsToCreate.forEach(({ from, to }) => {
                    ws.send(JSON.stringify({
                        type: 'AddSyncConnection',
                        from: from,
                        to: to,
                        team_id: selectedTeamId,
                        interval_secs: 5,
                        sync_now: true
                    }));
                });
                
                // Show single notification after operations are sent
                setTimeout(() => {
                    suppressNotifications = false;
                    showNotification(`Creating ${connectionCount} sync connections between team members`, 'success', 'Auto-Sync');
                }, 100);
            }
        }

        function syncAllMembersFromOwner() {
            if (!selectedTeamId) {
                showNotification('Please select a team first from the dropdown.', 'warning');
                return;
            }

            const team = teams.get(selectedTeamId);
            if (!team) {
                showNotification('Selected team not found.', 'error');
                return;
            }

            // Find the owner node
            const ownerNode = nodes.get(team.owner_node_id);
            if (!ownerNode || ownerNode.status !== 'Running') {
                showNotification('Team owner node is not running. Cannot create sync connections.', 'error');
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
                showNotification('No other running team members found. Add more nodes to the team first.', 'warning');
                return;
            }

            const connectionCount = memberNodes.length;
            const ownerName = ownerNode.name;
            const memberNames = memberNodes.map(n => n.name).join(', ');

            // Skip confirmation if called from quick setup (notifications are already suppressed)
            const skipConfirm = window.pendingQuickSetup && window.pendingQuickSetup.isQuickSetup;
            
            if (skipConfirm || confirm(`Make ${ownerName} (Owner) a sync peer for ${connectionCount} team members?\n\nThis will create sync connections FROM each member TO the owner:\n${memberNames}\n\nThis ensures all members receive commands from the owner.`)) {
                // Only suppress if not already suppressed
                const wasSuppressed = suppressNotifications;
                if (!wasSuppressed) {
                    suppressNotifications = true;
                }
                
                // Create sync connections from each member to the owner
                // This means each member will sync FROM the owner (receive owner's commands)
                memberNodes.forEach(memberNode => {
                    ws.send(JSON.stringify({
                        type: 'AddSyncConnection',
                        from: memberNode.id,      // Member syncs FROM owner
                        to: ownerNode.id,         // TO the owner
                        team_id: selectedTeamId,
                        interval_secs: 5,
                        sync_now: true
                    }));
                });

                // Only show notification if not called from quick setup
                if (!wasSuppressed) {
                    setTimeout(() => {
                        suppressNotifications = false;
                        showNotification(`Creating ${connectionCount} sync connections to owner ${ownerName}`, 'success', 'Owner Sync');
                    }, 100);
                }
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
                showNotification('No running nodes available. Place and start some nodes first.', 'warning');
                return;
            }

            const teamName = prompt(`Quick Team Setup\n\nThis will:\n1. Create a new team with ${runningNodes[0].name} as owner\n2. Add all ${runningNodes.length} running nodes to the team\n3. Set up sync connections from members to owner\n\nEnter team name (or cancel):`);
            
            if (!teamName || !teamName.trim()) {
                return;
            }

            // Suppress notifications during setup
            suppressNotifications = true;
            
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
                allNodes: runningNodes,
                isQuickSetup: true  // Add flag to identify quick setup
            };
        }

        // Helper function to complete quick setup after team creation
        function completeQuickSetup(teamId) {
            const setup = window.pendingQuickSetup;
            if (!setup) return;

            // Step 2: Add remaining nodes to team
            const nodesToAdd = setup.allNodes.filter(node => node.id !== setup.ownerNodeId);
            if (nodesToAdd.length > 0) {
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
            }

            // Step 3: Create sync connections (with delay to let joins complete)
            setTimeout(() => {
                // Set the team as selected
                selectedTeamId = teamId;
                document.getElementById('teamSelector').value = teamId;
                updateSelectedTeamInfo();

                // Make owner sync peer for all members
                setTimeout(() => {
                    syncAllMembersFromOwner();
                    
                    // Re-enable notifications and show final success
                    setTimeout(() => {
                        suppressNotifications = false;
                        showNotification(`Team "${setup.teamName}" setup complete with ${setup.allNodes.length} nodes!`, 'success', 'Quick Team Setup');
                        
                        // Clear pending setup AFTER everything is done
                        delete window.pendingQuickSetup;
                    }, 500);
                }, 2000); // Wait for joins to complete
            }, 1000);
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
                showNotification('This node is not part of any team. Join a team first to send messages.', 'warning');
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
            bubble.dataset.nodeId = nodeId; // Store node ID for position updates
            
            // Position bubble above node (convert world coordinates to screen)
            updateMessageBubblePosition(bubble, node);
            
            document.body.appendChild(bubble);
            
            // Store bubble reference
            if (!messageBubbles.has(nodeId)) {
                messageBubbles.set(nodeId, []);
            }
            messageBubbles.get(nodeId).push(bubble);
            
            // Animate in
            setTimeout(() => {
                bubble.classList.add('show');
            }, 100);
            
            // Remove after 6 seconds (longer duration for easier visibility)
            setTimeout(() => {
                bubble.classList.remove('show');
                setTimeout(() => {
                    if (bubble.parentNode) {
                        document.body.removeChild(bubble);
                    }
                    // Remove from tracking
                    const bubbles = messageBubbles.get(nodeId);
                    if (bubbles) {
                        const index = bubbles.indexOf(bubble);
                        if (index > -1) {
                            bubbles.splice(index, 1);
                        }
                        if (bubbles.length === 0) {
                            messageBubbles.delete(nodeId);
                        }
                    }
                }, 400);
            }, 6000);
        }
        
        function updateMessageBubblePosition(bubble, node) {
            if (!canvas) return;
            const rect = canvas.getBoundingClientRect();
            const screenPos = worldToScreen(node.position.x, node.position.y);
            bubble.style.left = (rect.left + screenPos.x - 100) + 'px';
            bubble.style.top = (rect.top + screenPos.y - 60) + 'px';
        }
        
        function updateAllMessageBubbles() {
            messageBubbles.forEach((bubbles, nodeId) => {
                const node = nodes.get(nodeId);
                if (node) {
                    bubbles.forEach(bubble => {
                        updateMessageBubblePosition(bubble, node);
                    });
                }
            });
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

        // Access Control event listeners
        document.getElementById('roleActingNodeSelector').addEventListener('change', updateAccessControlButtonStates);
        document.getElementById('roleTargetNodeSelector').addEventListener('change', updateAccessControlButtonStates);
        document.getElementById('roleTeamSelector').addEventListener('change', updateAccessControlButtonStates);
        document.getElementById('newRoleSelector').addEventListener('change', updateAccessControlButtonStates);
        
        document.getElementById('deviceRemovalNodeSelector').addEventListener('change', updateDeviceRemovalButtonState);
        document.getElementById('deviceRemovalTeamSelector').addEventListener('change', updateDeviceRemovalButtonState);
        
        // Removed overviewTeamSelector - Team Overview section was removed
        
        // Sync Peer event listeners
        document.getElementById('syncFromNodeSelector').addEventListener('change', updateSyncButtonState);
        document.getElementById('syncToNodeSelector').addEventListener('change', updateSyncButtonState);
        document.getElementById('syncTeamSelector').addEventListener('change', updateSyncButtonState);
        document.getElementById('syncInterval').addEventListener('input', updateSyncButtonState);
        document.getElementById('customSyncUrl').addEventListener('input', updateSyncButtonState);
        
        // Tool Arguments event listeners
        document.getElementById('connectRandomInterval').addEventListener('change', function() {
            const intervalInput = document.getElementById('connectInterval');
            if (this.checked) {
                intervalInput.disabled = true;
                intervalInput.placeholder = 'Random (1-10s)';
            } else {
                intervalInput.disabled = false;
                intervalInput.placeholder = '5';
            }
        });

        // Access Control Functions
        function assignRoleFromUI() {
            const actingNodeId = document.getElementById('roleActingNodeSelector').value;
            const targetNodeId = document.getElementById('roleTargetNodeSelector').value;
            const teamId = document.getElementById('roleTeamSelector').value;
            const role = document.getElementById('newRoleSelector').value;
            
            if (!actingNodeId || !targetNodeId || !teamId || !role) {
                showNotification('Please select acting node, target node, team, and role', 'warning');
                return;
            }
            
            if (actingNodeId === targetNodeId) {
                showNotification('Acting node and target node cannot be the same', 'warning');
                return;
            }
            
            const payload = {
                type: 'AssignRole',
                node_id: actingNodeId,
                target_node_id: targetNodeId,
                team_id: teamId,
                role: role
            };
            
            if (ws && ws.readyState === WebSocket.OPEN) {
                console.log('Sending AssignRole message:', payload);
                ws.send(JSON.stringify(payload));
                showNotification(`Assigning ${role} role to target node`, 'success');
            } else {
                showNotification('WebSocket not connected', 'error');
            }
        }
        
        function revokeRoleFromUI() {
            const actingNodeId = document.getElementById('roleActingNodeSelector').value;
            const targetNodeId = document.getElementById('roleTargetNodeSelector').value;
            const teamId = document.getElementById('roleTeamSelector').value;
            const role = document.getElementById('newRoleSelector').value;
            
            if (!actingNodeId || !targetNodeId || !teamId || !role) {
                showNotification('Please select acting node, target node, team, and role', 'warning');
                return;
            }
            
            const payload = {
                type: 'RevokeRole',
                node_id: actingNodeId,
                target_node_id: targetNodeId,
                team_id: teamId,
                role: role
            };
            
            if (ws && ws.readyState === WebSocket.OPEN) {
                console.log('Sending RevokeRole message:', payload);
                ws.send(JSON.stringify(payload));
                showNotification(`Revoking ${role} role from target node`, 'success');
            } else {
                showNotification('WebSocket not connected', 'error');
            }
        }
        
        function removeDeviceFromTeamUI() {
            const nodeId = document.getElementById('deviceRemovalNodeSelector').value;
            const teamId = document.getElementById('deviceRemovalTeamSelector').value;
            
            if (!nodeId || !teamId) {
                showNotification('Please select device and team', 'warning');
                return;
            }
            
            if (!confirm('Are you sure you want to remove this device from the team? This action cannot be undone.')) {
                return;
            }
            
            const payload = {
                type: 'RemoveDeviceFromTeam',
                node_id: nodeId,
                team_id: teamId
            };
            
            if (ws && ws.readyState === WebSocket.OPEN) {
                console.log('Sending RemoveDeviceFromTeam message:', payload);
                ws.send(JSON.stringify(payload));
                showNotification('Removing device from team', 'success');
            } else {
                showNotification('WebSocket not connected', 'error');
            }
        }
        
        // Removed showTeamOverview - Query Team Roles is not a real endpoint
        
        // Sync Configuration Functions
        function toggleAdvancedMode() {
            const advancedOptions = document.getElementById('advancedSyncOptions');
            const isChecked = document.getElementById('advancedMode').checked;
            advancedOptions.style.display = isChecked ? 'block' : 'none';
            
            // Clear/update fields based on mode
            if (isChecked) {
                document.getElementById('syncToNodeSelector').disabled = true;
                document.getElementById('syncToNodeSelector').value = '';
            } else {
                document.getElementById('syncToNodeSelector').disabled = false;
                document.getElementById('customSyncUrl').value = '';
            }
            updateSyncButtonState();
        }
        
        function addSyncConnectionFromUI() {
            const fromNodeId = document.getElementById('syncFromNodeSelector').value;
            const teamId = document.getElementById('syncTeamSelector').value;
            const interval = parseInt(document.getElementById('syncInterval').value) || 5;
            const syncNow = document.getElementById('syncNowFlag').checked;
            const advancedMode = document.getElementById('advancedMode').checked;
            
            if (!fromNodeId || !teamId) {
                showNotification('Please select source node and team', 'warning');
                return;
            }
            
            if (advancedMode) {
                // Advanced mode - use custom URL
                const customUrl = document.getElementById('customSyncUrl').value.trim();
                if (!customUrl) {
                    showNotification('Please enter a custom URL', 'warning');
                    return;
                }
                
                // Extract host and port from URL
                let address;
                try {
                    const url = new URL(customUrl);
                    address = url.hostname + ':' + (url.port || (url.protocol === 'https:' ? '443' : '80'));
                } catch (e) {
                    // Fallback for simple IP:Port format
                    if (customUrl.includes(':') && customUrl.match(/^[a-zA-Z0-9.-]+:\d+$/)) {
                        address = customUrl;
                    } else {
                        showNotification('Please enter a valid URL or IP:Port', 'warning');
                        return;
                    }
                }
                
                // For custom URLs, we'll use the REST API directly
                // This would need backend support to add external sync peers
                showNotification('Adding external sync peer: ' + address, 'info');
                // TODO: Implement backend support for external sync peers
                
            } else {
                // Normal mode - sync between visualization nodes
                const toNodeId = document.getElementById('syncToNodeSelector').value;
                if (!toNodeId) {
                    showNotification('Please select target node', 'warning');
                    return;
                }
                
                if (fromNodeId === toNodeId) {
                    showNotification('Source and target nodes cannot be the same', 'warning');
                    return;
                }
                
                // Check if nodes are in the same team
                const fromNode = nodes.get(fromNodeId);
                const toNode = nodes.get(toNodeId);
                
                if (!fromNode || !toNode) {
                    showNotification('Selected nodes not found', 'error');
                    return;
                }
                
                const fromInTeam = fromNode.teams && fromNode.teams.some(t => t.id === teamId);
                const toInTeam = toNode.teams && toNode.teams.some(t => t.id === teamId);
                
                if (!fromInTeam || !toInTeam) {
                    showNotification('Both nodes must be members of the selected team', 'warning');
                    return;
                }
                
                // Add sync connection
                const payload = {
                    type: 'AddSyncConnection',
                    from: fromNodeId,
                    to: toNodeId,
                    team_id: teamId,
                    interval_secs: interval,
                    sync_now: syncNow
                };
                
                if (ws && ws.readyState === WebSocket.OPEN) {
                    console.log('Adding sync connection:', payload);
                    ws.send(JSON.stringify(payload));
                    showNotification('Adding sync connection', 'success');
                } else {
                    showNotification('WebSocket not connected', 'error');
                }
            }
        }
        
        // Removed loadSyncPeers function - UI simplified
        
        // Removed triggerManualSync function - UI simplified
        
        function removeSyncPeer(address, teamId) {
            const payload = {
                type: 'RemoveSyncPeer',
                address: address,
                teamId: teamId
            };
            
            if (ws && ws.readyState === WebSocket.OPEN) {
                ws.send(JSON.stringify(payload));
                showNotification('Removing sync peer', 'success');
                // Reload the list
                loadSyncPeers();
            } else {
                showNotification('WebSocket not connected', 'error');
            }
        }
        
        function updateSyncPeerConfig(address, teamId, newInterval) {
            const payload = {
                type: 'UpdateSyncPeerConfig',
                address: address,
                teamId: teamId,
                intervalSecs: parseInt(newInterval),
                syncNow: false
            };
            
            if (ws && ws.readyState === WebSocket.OPEN) {
                ws.send(JSON.stringify(payload));
                showNotification('Updating sync peer configuration', 'success');
            } else {
                showNotification('WebSocket not connected', 'error');
            }
        }
        
        // Update selector validation functions
        function updateAccessControlButtonStates() {
            const actingNodeSelected = document.getElementById('roleActingNodeSelector').value;
            const targetNodeSelected = document.getElementById('roleTargetNodeSelector').value;
            const teamSelected = document.getElementById('roleTeamSelector').value;
            const roleSelected = document.getElementById('newRoleSelector').value;
            
            const allSelected = actingNodeSelected && targetNodeSelected && teamSelected && roleSelected;
            const sameNode = actingNodeSelected === targetNodeSelected && actingNodeSelected !== '';
            
            document.getElementById('assignRoleBtn').disabled = !allSelected || sameNode;
            document.getElementById('revokeRoleBtn').disabled = !allSelected || sameNode;
        }
        
        function updateDeviceRemovalButtonState() {
            const nodeSelected = document.getElementById('deviceRemovalNodeSelector').value;
            const teamSelected = document.getElementById('deviceRemovalTeamSelector').value;
            
            document.getElementById('removeDeviceBtn').disabled = !nodeSelected || !teamSelected;
        }
        
        // Removed updateTeamOverviewButtonState - Team Overview section was removed
        
        function updateSyncButtonState() {
            const fromNodeSelected = document.getElementById('syncFromNodeSelector').value;
            const teamSelected = document.getElementById('syncTeamSelector').value;
            const advancedMode = document.getElementById('advancedMode').checked;
            const toNodeSelected = document.getElementById('syncToNodeSelector').value;
            const customUrlEntered = document.getElementById('customSyncUrl').value.trim();
            
            let isValid = false;
            
            if (fromNodeSelected && teamSelected) {
                if (advancedMode) {
                    isValid = customUrlEntered.length > 0;
                } else {
                    isValid = toNodeSelected.length > 0 && fromNodeSelected !== toNodeSelected;
                }
            }
            
            document.getElementById('addSyncBtn').disabled = !isValid;
        }
        
        // Update node selectors for new UI elements
        function updateAccessControlSelectors() {
            // Role acting node selector
            const roleActingNodeSelector = document.getElementById('roleActingNodeSelector');
            const currentActingNodeValue = roleActingNodeSelector.value;
            roleActingNodeSelector.innerHTML = '<option value="">-- Select Acting Node --</option>';
            nodes.forEach((node, id) => {
                if (node.status === 'Running') {
                    const option = document.createElement('option');
                    option.value = id;
                    option.textContent = node.name || `Node ${id.slice(0, 8)}`;
                    roleActingNodeSelector.appendChild(option);
                }
            });
            if (currentActingNodeValue && nodes.has(currentActingNodeValue)) {
                roleActingNodeSelector.value = currentActingNodeValue;
            }
            
            // Role target node selector
            const roleTargetNodeSelector = document.getElementById('roleTargetNodeSelector');
            const currentTargetNodeValue = roleTargetNodeSelector.value;
            roleTargetNodeSelector.innerHTML = '<option value="">-- Select Target Node --</option>';
            nodes.forEach((node, id) => {
                if (node.status === 'Running') {
                    const option = document.createElement('option');
                    option.value = id;
                    option.textContent = node.name || `Node ${id.slice(0, 8)}`;
                    roleTargetNodeSelector.appendChild(option);
                }
            });
            if (currentTargetNodeValue && nodes.has(currentTargetNodeValue)) {
                roleTargetNodeSelector.value = currentTargetNodeValue;
            }
            
            // Device removal selector
            const deviceRemovalSelector = document.getElementById('deviceRemovalNodeSelector');
            const currentRemovalNodeValue = deviceRemovalSelector.value;
            deviceRemovalSelector.innerHTML = '<option value="">-- Select Device --</option>';
            nodes.forEach((node, id) => {
                if (node.status === 'Running') {
                    const option = document.createElement('option');
                    option.value = id;
                    option.textContent = node.name || `Node ${id.slice(0, 8)}`;
                    deviceRemovalSelector.appendChild(option);
                }
            });
            if (currentRemovalNodeValue && nodes.has(currentRemovalNodeValue)) {
                deviceRemovalSelector.value = currentRemovalNodeValue;
            }
            
            // Team selectors
            const teamSelectors = [
                'roleTeamSelector',
                'deviceRemovalTeamSelector'
            ];
            
            teamSelectors.forEach(selectorId => {
                const selector = document.getElementById(selectorId);
                const currentValue = selector.value;
                selector.innerHTML = '<option value="">-- Select Team --</option>';
                teams.forEach((team, id) => {
                    const option = document.createElement('option');
                    option.value = id;
                    option.textContent = team.name || `Team ${id.slice(0, 8)}`;
                    selector.appendChild(option);
                });
                if (currentValue && teams.has(currentValue)) {
                    selector.value = currentValue;
                }
            });
        }
        
        function updateSyncConfigSelectors() {
            // From node selector
            const syncFromNodeSelector = document.getElementById('syncFromNodeSelector');
            const currentFromValue = syncFromNodeSelector.value;
            syncFromNodeSelector.innerHTML = '<option value="">-- Select Source Node --</option>';
            nodes.forEach((node, id) => {
                if (node.status === 'Running') {
                    const option = document.createElement('option');
                    option.value = id;
                    option.textContent = node.name || `Node ${id.slice(0, 8)}`;
                    syncFromNodeSelector.appendChild(option);
                }
            });
            if (currentFromValue && nodes.has(currentFromValue)) {
                syncFromNodeSelector.value = currentFromValue;
            }
            
            // To node selector  
            const syncToNodeSelector = document.getElementById('syncToNodeSelector');
            const currentToValue = syncToNodeSelector.value;
            syncToNodeSelector.innerHTML = '<option value="">-- Select Target Node --</option>';
            nodes.forEach((node, id) => {
                if (node.status === 'Running') {
                    const option = document.createElement('option');
                    option.value = id;
                    option.textContent = node.name || `Node ${id.slice(0, 8)}`;
                    syncToNodeSelector.appendChild(option);
                }
            });
            if (currentToValue && nodes.has(currentToValue)) {
                syncToNodeSelector.value = currentToValue;
            }
            
            // Team selector
            const syncTeamSelector = document.getElementById('syncTeamSelector');
            const currentTeamValue = syncTeamSelector.value;
            syncTeamSelector.innerHTML = '<option value="">-- Select Team --</option>';
            teams.forEach((team, id) => {
                const option = document.createElement('option');
                option.value = id;
                option.textContent = team.name || `Team ${id.slice(0, 8)}`;
                syncTeamSelector.appendChild(option);
            });
            if (currentTeamValue && teams.has(currentTeamValue)) {
                syncTeamSelector.value = currentTeamValue;
            }
            
            updateSyncButtonState();
        }

        // Initialize
        initializeCanvas();
        setupCanvasEventListeners();
        initializeTexturePacks();
        initializeResizers();
        initializeDrawers();
        loadCustomTexturePack();
        loadAllTexturePacks();
        loadPreferences();
        connectWebSocket();
    </script>
</body>
</html>"#)
}