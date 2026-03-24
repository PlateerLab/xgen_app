'use client';

import React, { useState, useRef, useEffect } from 'react';
import { isTauri } from '@/app/_common/api/core/platform';
import { getAuthCookie } from '@/app/_common/utils/cookieUtils';
import styles from './CliPanel.module.scss';

interface Message {
    role: 'user' | 'assistant' | 'system';
    content: string;
    toolCalls?: { name: string; input: any }[];
    isStreaming?: boolean;
}

interface Provider {
    name: string;
    model: string;
    configured: boolean;
    available: boolean;
}

interface CliStreamEvent {
    sessionId: string;
    eventType: string;
    data: any;
}

const CliPanel: React.FC = () => {
    const [messages, setMessages] = useState<Message[]>([
        { role: 'system', content: 'XGEN AI CLI에 오신 것을 환영합니다. 자연어로 명령을 입력하세요.\n예: "워크플로우 목록 보여줘", "LLM 상태 확인해줘"' },
    ]);
    const [input, setInput] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const [providers, setProviders] = useState<Provider[]>([]);
    const [selectedProvider, setSelectedProvider] = useState('anthropic');
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLTextAreaElement>(null);
    const streamingTextRef = useRef('');

    const scrollToBottom = () => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    };

    useEffect(() => {
        scrollToBottom();
    }, [messages]);

    // Load available providers on mount
    useEffect(() => {
        if (!isTauri()) return;
        (async () => {
            try {
                const { invoke } = await import('@tauri-apps/api/core');
                const token = getAuthCookie('access_token') || undefined;
                const list = await invoke<Provider[]>('cli_list_providers', { xgenToken: token });
                setProviders(list.filter(p => p.configured && p.available));
            } catch (e) {
                console.error('Failed to load providers:', e);
            }
        })();
    }, []);

    // Listen for streaming events from Tauri backend
    useEffect(() => {
        if (!isTauri()) return;

        let unlisten: (() => void) | null = null;

        const setup = async () => {
            const { listen } = await import('@tauri-apps/api/event');
            unlisten = await listen<CliStreamEvent>('cli:event', (event) => {
                const { eventType, data } = event.payload;

                switch (eventType) {
                    case 'token':
                        streamingTextRef.current += (typeof data === 'string' ? data : '');
                        setMessages(prev => {
                            const updated = [...prev];
                            const last = updated[updated.length - 1];
                            if (last && last.role === 'assistant' && last.isStreaming) {
                                updated[updated.length - 1] = {
                                    ...last,
                                    content: streamingTextRef.current,
                                };
                            }
                            return updated;
                        });
                        break;

                    case 'tool_call':
                    case 'tool_call_start':
                        setMessages(prev => {
                            const updated = [...prev];
                            const last = updated[updated.length - 1];
                            if (last && last.role === 'assistant') {
                                const tools = last.toolCalls || [];
                                updated[updated.length - 1] = {
                                    ...last,
                                    toolCalls: [...tools, { name: data.name, input: data.input }],
                                };
                            }
                            return updated;
                        });
                        break;

                    case 'tool_result':
                        // Tool result received, streaming will continue
                        break;

                    case 'done':
                        setMessages(prev => {
                            const updated = [...prev];
                            const last = updated[updated.length - 1];
                            if (last && last.role === 'assistant') {
                                updated[updated.length - 1] = { ...last, isStreaming: false };
                            }
                            return updated;
                        });
                        setIsLoading(false);
                        break;

                    case 'error':
                        setMessages(prev => [...prev, {
                            role: 'system' as const,
                            content: `오류: ${typeof data === 'string' ? data : JSON.stringify(data)}`,
                        }]);
                        setIsLoading(false);
                        break;
                }
            });
        };

        setup();
        return () => { unlisten?.(); };
    }, []);

    const handleSend = async () => {
        if (!input.trim() || isLoading) return;

        const userMessage = input.trim();
        setInput('');
        setMessages(prev => [...prev, { role: 'user', content: userMessage }]);
        setIsLoading(true);
        streamingTextRef.current = '';

        // Add streaming placeholder
        setMessages(prev => [...prev, { role: 'assistant', content: '', isStreaming: true }]);

        if (!isTauri()) {
            setMessages(prev => {
                const updated = [...prev];
                updated[updated.length - 1] = {
                    role: 'system',
                    content: 'AI CLI는 데스크톱 앱에서만 사용 가능합니다.',
                };
                return updated;
            });
            setIsLoading(false);
            return;
        }

        try {
            const { invoke } = await import('@tauri-apps/api/core');
            // Pass XGEN auth token from cookie
            const token = getAuthCookie('access_token') || undefined;
            await invoke('cli_send_message', {
                message: userMessage,
                xgenToken: token,
                provider: selectedProvider,
            });
            // Response handled by event listener
        } catch (e: any) {
            setMessages(prev => {
                const updated = [...prev];
                updated[updated.length - 1] = {
                    role: 'system',
                    content: `오류: ${e}`,
                    isStreaming: false,
                };
                return updated;
            });
            setIsLoading(false);
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSend();
        }
    };

    const handleClear = async () => {
        if (!isTauri()) return;
        try {
            const { invoke } = await import('@tauri-apps/api/core');
            await invoke('cli_clear_session');
            setMessages([{ role: 'system', content: '세션이 초기화되었습니다.' }]);
        } catch (e) {
            console.error('Failed to clear session:', e);
        }
    };

    return (
        <div className={styles.cliPanel}>
            <div className={styles.header}>
                <span className={styles.headerTitle}>AI CLI</span>
                <div className={styles.headerControls}>
                    {providers.length > 0 && (
                        <select
                            className={styles.providerSelect}
                            value={selectedProvider}
                            onChange={(e) => setSelectedProvider(e.target.value)}
                            disabled={isLoading}
                        >
                            {providers.map(p => (
                                <option key={p.name} value={p.name}>
                                    {p.name} ({p.model})
                                </option>
                            ))}
                        </select>
                    )}
                    <button onClick={handleClear} className={styles.clearButton} title="세션 초기화">
                        ↺
                    </button>
                </div>
            </div>

            <div className={styles.messages}>
                {messages.map((msg, i) => (
                    <div key={i} className={`${styles.message} ${styles[msg.role]}`}>
                        {msg.role === 'user' && <div className={styles.messageLabel}>You</div>}
                        {msg.role === 'assistant' && <div className={styles.messageLabel}>AI</div>}
                        {msg.role === 'system' && <div className={styles.messageLabel}>System</div>}

                        <div className={styles.messageContent}>
                            {msg.content || (msg.isStreaming ? '...' : '')}
                        </div>

                        {msg.toolCalls && msg.toolCalls.length > 0 && (
                            <div className={styles.toolCalls}>
                                {msg.toolCalls.map((tool, j) => (
                                    <div key={j} className={styles.toolCall}>
                                        <span className={styles.toolIcon}>⚡</span>
                                        <span className={styles.toolName}>{tool.name}</span>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                ))}
                <div ref={messagesEndRef} />
            </div>

            <div className={styles.inputArea}>
                <textarea
                    ref={inputRef}
                    value={input}
                    onChange={(e) => setInput(e.target.value)}
                    onKeyDown={handleKeyDown}
                    placeholder="명령을 입력하세요... (예: 워크플로우 목록 보여줘)"
                    className={styles.input}
                    rows={1}
                    disabled={isLoading}
                />
                <button
                    onClick={handleSend}
                    className={styles.sendButton}
                    disabled={isLoading || !input.trim()}
                >
                    {isLoading ? '...' : '→'}
                </button>
            </div>
        </div>
    );
};

export default CliPanel;
