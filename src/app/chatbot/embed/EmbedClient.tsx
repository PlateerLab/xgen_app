'use client';
import React, { useState, useEffect } from 'react';
import ChatInterface from '@/app/main/chatSection/components/ChatInterface';
import styles from '@/app/chatbot/embed/[chatId]/Embed.module.scss';

type Params = { chatId: string };

export default function EmbedClient({ params }: { params: Params }) {
    const chatId = params?.chatId;
    const [workflow, setWorkflow] = useState<any | null>(null);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        const workflowNameFromUrl = typeof window !== 'undefined' ? new URLSearchParams(window.location.search).get('workflowName') : null;
        if (!chatId || !workflowNameFromUrl) {
            setLoading(false);
            return;
        }

        const fetchedWorkflow = {
            id: chatId,
            name: workflowNameFromUrl,
            filename: workflowNameFromUrl,
            author: 'Unknown',
            nodeCount: 0,
            status: 'active' as const,
        };

        setWorkflow(fetchedWorkflow);
        setLoading(false);
    }, [chatId]);

    if (loading || !workflow) {
        return <div className={styles.loader}></div>;
    }

    return (
        <div className={styles.embedContainer}>
            <ChatInterface
                mode="deploy"
                onBack={() => {}}
                onChatStarted={() => {}}
                hideBackButton={true}
                existingChatData={undefined}
                workflow={workflow}
                user_id={chatId}
            />
        </div>
    );
}


