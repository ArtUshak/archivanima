/// <amd-module name='archivanima/ajax'/>

import { Either, combine, isLeft, left, mapLeft, mapRight, right } from "archivanima/utils";

export enum ConnectionError {
    Abort,
    Error,
    Timeout
};

export interface HTTPResult {
    status: number,
    body: unknown
}

export type RequestError = Either<ConnectionError, HTTPResult>;

/**
 * CSRF token
 * @type {string}
 */
const csrfToken: string | null = document.head.querySelector('meta[name="csrf-token"]')?.getAttribute('content') ?? null;

function ajaxRequest(
    method: 'GET' | 'HEAD' | 'POST' | 'PUT', url: string,
    body?: Document | XMLHttpRequestBodyInit | null,
    onProgress?: (loaded: number, total: number) => void,
    extraHeaders?: { [header: string]: string },
    responseType: XMLHttpRequestResponseType = 'json'
): Promise<Either<HTTPResult, RequestError>> {
    const xhr = new XMLHttpRequest();
    xhr.open(method, url);
    xhr.responseType = responseType;
    for (let key in extraHeaders) {
        xhr.setRequestHeader(key, extraHeaders[key]);
    }

    return new Promise<Either<HTTPResult, RequestError>>(
        (resolve: (value: Either<HTTPResult, RequestError>) => void) => {
            xhr.upload.onprogress = function (this: XMLHttpRequest, event: ProgressEvent) { onProgress?.call(this, event.loaded, event.total) };
            xhr.onabort = () => resolve(right(right(ConnectionError.Abort)));
            xhr.ontimeout = () => resolve(right(right(ConnectionError.Timeout)));
            xhr.onerror = () => resolve(right(right(ConnectionError.Error)));
            xhr.onload = () => {
                if ((xhr.status < 200) || (xhr.status >= 300)) {
                    resolve(right(left({ status: xhr.status, body: xhr.response })));
                } else {
                    resolve(left({ status: xhr.status, body: xhr.response }));
                }
            }

            xhr.send(body);
        }
    );
}

export async function ajaxGet(
    url: string, onProgress?: (loaded: number, total: number) => void
): Promise<Either<HTTPResult, RequestError>> {
    return ajaxRequest('GET', url, undefined, onProgress);
}

export async function ajaxHead(
    url: string, onProgress?: (loaded: number, total: number) => void
): Promise<Either<number, RequestError>> {
    const result = await ajaxRequest('HEAD', url, undefined, onProgress, {}, 'text');
    return combine(mapRight(
        mapLeft(result, result => result.status),
        error => mapRight(error, result => result.status)
    ));
}

export function ajaxPost(
    url: string, data: Document | XMLHttpRequestBodyInit | null,
    onProgress?: (loaded: number, total: number) => void,
    extraHeaders: { [header: string]: string } = {}
): Promise<Either<HTTPResult, RequestError>> {
    const extendedHeaders = (csrfToken != null)
        ? {
            'X-CSRF-Token': csrfToken,
            ...extraHeaders
        }
        : extraHeaders;

    return ajaxRequest('POST', url, data, onProgress, extendedHeaders);
}

export function ajaxPostJSON(
    url: string, data: unknown,
    onProgress?: (loaded: number, total: number) => void,
    extraHeaders: { [header: string]: string } = {}
): Promise<Either<HTTPResult, RequestError>> {
    return ajaxPost(
        url, JSON.stringify(data), onProgress,
        {
            'Content-Type': 'application/json',
            ...extraHeaders
        }
    );
}

export function ajaxPut(
    url: string, data: Document | XMLHttpRequestBodyInit | null,
    onProgress?: (loaded: number, total: number) => void,
    extraHeaders: { [header: string]: string } = {}
): Promise<Either<HTTPResult, RequestError>> {
    const extendedHeaders = (csrfToken != null)
        ? {
            'X-CSRF-Token': csrfToken,
            ...extraHeaders
        }
        : extraHeaders;

    return ajaxRequest('PUT', url, data, onProgress, extendedHeaders);
}
