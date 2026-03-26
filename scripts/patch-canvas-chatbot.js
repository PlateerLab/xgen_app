#!/usr/bin/env node
/**
 * Canvas에 AI 챗봇 이벤트 핸들러를 패치하는 스크립트
 *
 * 1. Canvas/index.tsx의 useImperativeHandle에 누락 함수 추가
 *    - addNodeAtPosition(nodeData, position)
 *    - deleteNodeById(nodeId)
 *    - addEdgeBetween(source, sourcePort, target, targetPort)
 *    - getAvailableNodeSpecs()
 *
 * 2. canvas/page.tsx에 canvas:command 이벤트 리스너 주입
 */
const fs = require('fs');
const path = require('path');

const frontendDir = process.argv[2];
if (!frontendDir) {
    console.error('Usage: node patch-canvas-chatbot.js <frontend-dir>');
    process.exit(1);
}

// ============================================================
// Part 1: Canvas/index.tsx — useImperativeHandle 확장
// ============================================================

const canvasPath = path.join(frontendDir,
    'src/app/components/pages/workflow/canvas/components/Canvas/index.tsx');

if (!fs.existsSync(canvasPath)) {
    console.log('[WARN] Canvas/index.tsx not found — skipping canvas patch');
    process.exit(0);
}

let canvasContent = fs.readFileSync(canvasPath, 'utf8');

if (canvasContent.includes('addNodeAtPosition')) {
    console.log('[INFO] Canvas already patched');
} else {
    console.log('[PATCH] Canvas/index.tsx — adding imperative handle functions...');

    // Insert before the closing "}));" of useImperativeHandle
    // Find: "updateNodeParameter: (nodeId..." block ending with "},\n    }));"
    canvasContent = canvasContent.replace(
        /updateNodeParameter: \(nodeId: string, paramId: string, value: string \| number \| boolean, skipHistory\?: boolean, label\?: string\): void => \{\s*\n\s*updateNodeParameter\(nodeId, paramId, value, skipHistory, label\);\s*\n\s*\},\s*\n\s*\}\)\);/,
        `updateNodeParameter: (nodeId: string, paramId: string, value: string | number | boolean, skipHistory?: boolean, label?: string): void => {
            updateNodeParameter(nodeId, paramId, value, skipHistory, label);
        },
        // === AI Chatbot: Canvas command API ===
        addNodeAtPosition: (nodeData: NodeData, position?: { x: number; y: number }): string => {
            const pos = position || { x: 200 + Math.random() * 400, y: 100 + Math.random() * 300 };
            const newNode = {
                id: nodeData.id + '-' + Date.now(),
                data: nodeData,
                position: pos,
                isExpanded: true,
            };
            addNode(newNode);
            return newNode.id;
        },
        deleteNodeById: (nodeId: string): boolean => {
            const node = nodesRef.current.find(n => n.id === nodeId);
            if (!node) return false;
            const connectedEdges = edgesRef.current.filter(
                e => e.source === nodeId || e.target === nodeId
            );
            deleteNode(nodeId, connectedEdges);
            return true;
        },
        addEdgeBetween: (sourceNode: string, sourcePort: string, targetNode: string, targetPort: string): string | null => {
            const edgeId = sourceNode + ':' + sourcePort + '->' + targetNode + ':' + targetPort;
            const newEdge = {
                id: edgeId,
                source: sourceNode,
                sourceHandle: sourcePort,
                target: targetNode,
                targetHandle: targetPort,
            };
            addEdge(newEdge);
            return edgeId;
        },
        removeEdgeById: (edgeId: string): boolean => {
            const edge = edgesRef.current.find(e => e.id === edgeId);
            if (!edge) return false;
            removeEdge(edgeId);
            return true;
        },
        getAvailableNodes: (): any[] => {
            return availableNodeSpecs || [];
        },
    }));`
    );

    // Verify edgesRef exists (it should, since edges are managed internally)
    if (!canvasContent.includes('edgesRef')) {
        // Add edgesRef if missing
        canvasContent = canvasContent.replace(
            /const nodesRef = useRef/,
            'const edgesRef = useRef(edges);\n    const nodesRef = useRef'
        );
        // Keep edgesRef in sync
        if (!canvasContent.includes('edgesRef.current = edges')) {
            canvasContent = canvasContent.replace(
                /nodesRef\.current = nodes;/,
                'nodesRef.current = nodes;\n    edgesRef.current = edges;'
            );
        }
    }

    fs.writeFileSync(canvasPath, canvasContent);
    console.log('[OK] Canvas/index.tsx patched');
}

// ============================================================
// Part 2: canvas/page.tsx — canvas:command 이벤트 리스너
// ============================================================

const pagePath = path.join(frontendDir, 'src/app/canvas/page.tsx');

if (!fs.existsSync(pagePath)) {
    console.log('[WARN] canvas/page.tsx not found — skipping page patch');
    process.exit(0);
}

let pageContent = fs.readFileSync(pagePath, 'utf8');

if (pageContent.includes('canvas:command')) {
    console.log('[INFO] canvas/page.tsx already patched');
    process.exit(0);
}

console.log('[PATCH] canvas/page.tsx — adding canvas:command event listener...');

// Find a useEffect that runs after canvas is ready, and add our listener
// Strategy: add a new useEffect after the imports and component setup

// 1. Add isTauri import if not present
if (!pageContent.includes('isTauri')) {
    pageContent = pageContent.replace(
        /(import.*from '@\/app\/canvas\/types';)/,
        "$1\nimport { isTauri } from '@/app/_common/api/core/platform';"
    );
}

// 2. Find a good place to inject the canvas:command useEffect
// After "const canvasRef = useRef<any>(null);" line
pageContent = pageContent.replace(
    /(const canvasRef = useRef<any>\(null\);)/,
    `$1

    // === AI Chatbot: Canvas command handler ===
    useEffect(() => {
        if (!isTauri()) return;
        let unlisten: (() => void) | null = null;

        import('@tauri-apps/api/event').then(({ listen, emit }) => {
            listen('canvas:command', async (event: any) => {
                const { requestId, action, params } = event.payload;
                const canvas = canvasRef.current;
                let result: any = { error: 'Canvas not ready' };

                if (canvas) {
                    try {
                        switch (action) {
                            case 'get_nodes': {
                                const state = canvas.getCanvasState();
                                result = (state.nodes || []).map((n: any) => ({
                                    id: n.id,
                                    type: n.data?.nodeName || n.data?.id,
                                    name: n.data?.name || n.data?.nodeName,
                                    position: n.position,
                                    parameters: (n.data?.parameters || []).map((p: any) => ({
                                        name: p.name, value: p.value, type: p.type
                                    })),
                                }));
                                break;
                            }
                            case 'get_available_nodes': {
                                const specs = canvas.getAvailableNodes?.() || [];
                                const category = params?.category;
                                const filtered = category
                                    ? specs.filter((s: any) => s.id?.startsWith(category))
                                    : specs;
                                result = filtered.map((s: any) => ({
                                    id: s.id,
                                    name: s.nodeName || s.name,
                                    category: s.id?.split('/')[0],
                                    inputs: (s.inputs || []).map((i: any) => i.name),
                                    outputs: (s.outputs || []).map((o: any) => o.name),
                                }));
                                break;
                            }
                            case 'add_node': {
                                const nodeType = params?.node_type;
                                const specs = canvas.getAvailableNodes?.() || [];
                                const spec = specs.find((s: any) => s.id === nodeType);
                                if (!spec) {
                                    result = { error: 'Node type not found: ' + nodeType };
                                } else {
                                    const nodeId = canvas.addNodeAtPosition(spec, params?.position);
                                    result = { success: true, node_id: nodeId };
                                }
                                break;
                            }
                            case 'remove_node': {
                                const success = canvas.deleteNodeById?.(params?.node_id);
                                result = { success: !!success };
                                break;
                            }
                            case 'connect': {
                                const edgeId = canvas.addEdgeBetween?.(
                                    params?.source_node, params?.source_port,
                                    params?.target_node, params?.target_port
                                );
                                result = edgeId ? { success: true, edge_id: edgeId } : { error: 'Failed to connect' };
                                break;
                            }
                            case 'disconnect': {
                                const success = canvas.removeEdgeById?.(params?.edge_id);
                                result = { success: !!success };
                                break;
                            }
                            case 'update_node_param': {
                                canvas.updateNodeParameter?.(
                                    params?.node_id, params?.param_name, params?.value
                                );
                                result = { success: true };
                                break;
                            }
                            case 'save': {
                                // Trigger the save button click programmatically
                                const saveBtn = document.querySelector('[data-testid="save-workflow"]') as HTMLButtonElement;
                                if (saveBtn) {
                                    saveBtn.click();
                                    result = { success: true };
                                } else {
                                    result = { error: 'Save button not found' };
                                }
                                break;
                            }
                            default:
                                result = { error: 'Unknown canvas action: ' + action };
                        }
                    } catch (err: any) {
                        result = { error: err.message || String(err) };
                    }
                }

                // Send result back to Rust
                emit('canvas:result', { requestId, result: JSON.stringify(result) });
            }).then(fn => { unlisten = fn; });
        }).catch(() => {});

        return () => { if (unlisten) unlisten(); };
    }, []);`
);

fs.writeFileSync(pagePath, pageContent);
console.log('[OK] canvas/page.tsx patched');
