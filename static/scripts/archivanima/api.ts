/// <amd-module name='archivanima/api'/>

import { RequestError, ajaxPost, ajaxPostJSON, ajaxPut } from 'archivanima/ajax';
import { Either, getRight, isRight, left, mapLeft, right, unwrapEitherOrThrow, unwrapOrThrow } from 'archivanima/utils';

function getFileExtension(
    fileName: string
): string | null {
    const fileNameDotPos = fileName.lastIndexOf('.');
    if (fileNameDotPos < 0) {
        return null;
    } else {
        return fileName.substring(fileNameDotPos + 1);
    };
}

export async function uploadFile(
    file: File, chunkSize: number, postId: number,
    onProgress: (id: number, uploadedSize: number, totalSize: number) => void
): Promise<Either<number, RequestError>> {
    const extension = getFileExtension(file.name);

    const result = await ajaxPostJSON(
        '/api/uploads/add',
        {
            size: file.size,
            extension: extension,
            post_id: postId
        }
    );
    if (isRight(result)) {
        return right(unwrapOrThrow(getRight(result)));
    }
    const id = <number>(<{ [s: string]: unknown }>(unwrapEitherOrThrow(result).body))['id'];

    onProgress(id, 0, file.size);

    let chunkStart = 0;
    while (chunkStart < file.size) {
        const chunkEnd = Math.min(chunkStart + chunkSize, file.size);
        const chunk = file.slice(chunkStart, chunkEnd);

        const result = await ajaxPut(
            `/api/uploads/by-id/${id}/upload-by-chunk`,
            chunk,
            (loaded: number, _total: number) => onProgress(id, loaded + chunkStart, file.size),
            {
                'content-range': `bytes ${chunkStart}-${chunkEnd - 1}/${file.size}`
            }
        );
        if (isRight(result)) {
            return right(unwrapOrThrow(getRight(result)));
        }

        chunkStart = chunkEnd;
        onProgress(id, chunkStart, file.size);
    }

    const result1 = await ajaxPostJSON(
        `/api/uploads/by-id/${id}/finalize`,
        {}
    );
    if (isRight(result1)) {
        return right(unwrapOrThrow(getRight(result1)));
    }

    return left(id);
}

export async function removeFile(
    id: number
): Promise<Either<void, RequestError>> {
    return mapLeft(
        await ajaxPost(
            `/api/uploads/by-id/${id}/remove`,
            null
        ),
        () => { }
    );
}

export interface PostResult {
    id: number;
    url: string;
}

export async function addPost(
    title: string, description: string, is_hidden: boolean, is_pinned: boolean | null, minAge: number | null,
): Promise<Either<PostResult, RequestError>> {
    const result = await ajaxPostJSON(
        '/api/posts/add',
        {
            title: title,
            description: description,
            is_hidden: is_hidden,
            is_pinned: is_pinned,
            min_age: minAge
        }
    );
    return mapLeft(
        result,
        (response) => {
            const typedResponse = <{ [s: string]: unknown }>(response.body);
            return {
                id: <number>typedResponse['id'],
                url: <string>typedResponse['url'],
            };
        }
    );
}

export async function editPost(
    id: number, title: string | null, description: string | null, is_hidden: boolean | null, is_pinned: boolean | null,
    minAge: number | null
): Promise<Either<void, RequestError>> {
    return mapLeft(
        await ajaxPostJSON(
            `/api/posts/by-id/${id}/edit`,
            {
                title: title,
                description: description,
                is_hidden: is_hidden,
                is_pinned: is_pinned,
                min_age: minAge
            }
        ),
        () => { }
    );
}
