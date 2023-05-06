/// <amd-module name='peresvet12/api'/>

import { ajaxPost, ajaxPostJSON, ajaxPut } from 'peresvet12/ajax';

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
    onProgress: (id: number, uploadedSize: number, totalSize: number) => void,
    onError: (xhr: JQueryXHR, textStatus: string, errorThrown: string) => void,
): Promise<number> {
    const extension = getFileExtension(file.name);

    const result = await ajaxPostJSON(
        '/api/uploads/add',
        {
            size: file.size,
            extension: extension,
            post_id: postId
        },
        {},
        {
            error: onError
        }
    );
    const id = <number>(<{ [s: string]: unknown }>result)['id'];

    onProgress(id, 0, file.size);

    let chunkStart = 0;
    while (chunkStart < file.size) {
        const chunkEnd = Math.min(chunkStart + chunkSize, file.size);
        const chunk = file.slice(chunkStart, chunkEnd);

        await ajaxPut(
            `/api/uploads/by-id/${id}/upload-by-chunk`,
            chunk,
            {
                'content-range': `bytes ${chunkStart}-${chunkEnd - 1}/${file.size}`
            },
            {
                error: onError,
                processData: false,
                contentType: false,
                xhr: () => {
                    var xhr = new XMLHttpRequest();
                    xhr.upload.addEventListener(
                        'progress',
                        function (evt) {
                            if (evt.lengthComputable) {
                                onProgress(id, chunkStart + evt.loaded, file.size);
                            }
                        },
                        false
                    );
                    return xhr;
                }
            }
        );

        chunkStart = chunkEnd;
        onProgress(id, chunkStart, file.size);
    }

    await ajaxPostJSON(
        `/api/uploads/by-id/${id}/finalize`,
        {},
        {},
        {
            error: onError
        }
    );

    return id;
}

export async function removeFile(
    id: number,
    onError: (xhr: JQueryXHR, textStatus: string, errorThrown: string) => void,
): Promise<void> {
    await ajaxPost(
        `/api/uploads/by-id/${id}/remove`,
        undefined,
        {},
        {
            error: onError
        }
    );
}

export interface PostResult {
    id: number;
    url: string;
}

export async function addPost(
    title: string, description: string, is_hidden: boolean, minAge: number | null,
    onError: (xhr: JQueryXHR, textStatus: string, errorThrown: string) => void,
): Promise<PostResult> {
    const result = await ajaxPostJSON(
        '/api/posts/add',
        {
            title: title,
            description: description,
            is_hidden: is_hidden,
            min_age: minAge
        },
        {},
        {
            error: onError
        }
    );
    const id = <number>(<{ [s: string]: unknown }>result)['id'];
    const url = <string>(<{ [s: string]: unknown }>result)['url'];

    return {
        id: id,
        url: url
    };
}

export async function editPost(
    id: number, title: string | null, description: string | null, is_hidden: boolean | null,
    minAge: number | null,
    onError: (xhr: JQueryXHR, textStatus: string, errorThrown: string) => void,
): Promise<void> {
    const result = await ajaxPostJSON(
        `/api/posts/by-id/${id}/edit`,
        {
            title: title,
            description: description,
            is_hidden: is_hidden,
            min_age: minAge
        },
        {},
        {
            error: onError
        }
    );
}
