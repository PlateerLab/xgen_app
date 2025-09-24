import React, { Suspense } from 'react';
import styles from './Embed.module.scss';
import EmbedClient from '../EmbedClient';

export async function generateStaticParams() {
    // 빌드 시 정적으로 생성할 chatId 목록을 반환합니다.
    // 최소한 데모용 chatId를 하나 등록하면 정적 export 빌드가 통과합니다.
    return [{ chatId: 'demo' }];
}

export default function Page(props: any) {
    const params = props?.params;
    return (
        <Suspense fallback={<div className={styles.loader}></div>}>
            <EmbedClient params={params} />
        </Suspense>
    );
}
