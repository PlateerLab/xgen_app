'use client';
import React, { useState, useEffect } from 'react';
import { useRouter } from 'next/navigation';
import ChatInterface from '@/app/main/chatSection/components/ChatInterface';
import { decryptUrlParams } from '@/app/_common/utils/urlEncryption';
import styles from '@/app/chatbot/[chatId]/StandaloneChat.module.scss';

export default function StandaloneChatClient({ params }: { params?: { chatId?: string } }) {
    const router = useRouter();
    const encryptedParams = params?.chatId as string | undefined;
    const workflowNameFromUrl = typeof window !== 'undefined' ? new URLSearchParams(window.location.search).get('workflowName') as string : null;
    const hideInputUI = typeof window !== 'undefined' ? new URLSearchParams(window.location.search).get('hideInput') === 'true' : false;

    const [workflow, setWorkflow] = useState<any | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [userId, setUserId] = useState<string>('');
    const [workflowName, setWorkflowName] = useState<string>('');

    useEffect(() => {
        const handleDecryption = async () => {
            if (!encryptedParams) {
                setError('잘못된 접근입니다. 암호화된 파라미터가 필요합니다.');
                setLoading(false);
                return;
            }

            try {
                const decryptedParams = await decryptUrlParams(encryptedParams);

                if (!decryptedParams) {
                    if (workflowNameFromUrl) {
                        setUserId(encryptedParams);
                        setWorkflowName(workflowNameFromUrl);
                    } else {
                        setError('URL 파라미터를 복호화할 수 없습니다.');
                        setLoading(false);
                        return;
                    }
                } else {
                    if (decryptedParams.message) {
                        setError(decryptedParams.message === 'This workflow is not deployed.'
                            ? '이 워크플로우는 배포되지 않았습니다.'
                            : decryptedParams.message);
                        setLoading(false);
                        return;
                    }
                    setUserId(decryptedParams.userId);
                    setWorkflowName(decryptedParams.workflowName);
                }

                const fetchWorkflow = async () => {
                    try {
                        setLoading(true);
                        const currentUserId = (decryptedParams && decryptedParams.userId) || userId;
                        const currentWorkflowName = (decryptedParams && decryptedParams.workflowName) || workflowName;

                        if (!currentUserId || !currentWorkflowName) {
                            setError('사용자 ID 또는 워크플로우 이름이 없습니다.');
                            return;
                        }

                        const fetchedWorkflow = {
                            id: currentUserId,
                            name: currentWorkflowName,
                            filename: currentWorkflowName,
                            author: 'Unknown',
                            nodeCount: 0,
                            status: 'active' as const,
                        };
                        setWorkflow(fetchedWorkflow);
                        setError(null);
                    } catch (err) {
                        console.error(err);
                        setError('워크플로우를 불러오는 데 실패했습니다. 파라미터를 확인해 주세요.');
                        setWorkflow(null);
                    } finally {
                        setLoading(false);
                    }
                };

                fetchWorkflow();
            } catch (error) {
                console.error('Decryption error:', error);
                setError('URL 파라미터 복호화 중 오류가 발생했습니다.');
                setLoading(false);
            }
        };

        handleDecryption();
    }, [encryptedParams]);

    if (loading) {
        return (
            <div className={styles.pageContainer}>
                <div className={styles.centeredMessage}>
                    <div className={styles.spinner}></div>
                    <p>채팅 인터페이스를 불러오는 중입니다...</p>
                </div>
            </div>
        );
    }

    if (error || !workflow) {
        return (
            <div className={styles.pageContainer}>
                <div className={styles.centeredMessage}>
                    <h2>오류</h2>
                    <p>{error || '워크플로우를 찾을 수 없습니다.'}</p>
                    <button onClick={() => router.push('/')} className={styles.homeButton}>
                        홈으로 돌아가기
                    </button>
                </div>
            </div>
        );
    }

    return (
        <div className={styles.mainContent}>
            <div className={styles.chatContainer}>
                <div className={styles.workflowSection}>
                    <ChatInterface
                        mode="deploy"
                        workflow={workflow}
                        onBack={() => { }}
                        onChatStarted={() => { }}
                        hideBackButton={true}
                        hideInputUI={hideInputUI}
                        existingChatData={undefined}
                        user_id={userId || workflow.id}
                    />
                </div>
            </div>
        </div>
    );
}


