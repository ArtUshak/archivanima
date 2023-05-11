/// <amd-module name='archivanima/ajax'/>

import * as $ from 'jquery';

/**
 * CSRF token
 * @type {string}
 */
const csrfToken: string | null = $('meta[name="csrf-token"]').attr('content') ?? null;

function ajaxReject(reject: (reason?: unknown) => void): (xhr: JQueryXHR, textStatus: string, errorThrown: string) => void {
    return function (_xhr: JQueryXHR, textStatus: string, errorThrown: string) {
        reject(new Error('AJAX error: error [' + errorThrown + '] status [' + textStatus + ']'));
    };
}

/**
 * Perform AJAX GET request
 * @param {string} url 
 * @param {unknown} data - data for request
 * @param {Object.<string, unknown>} miscParams - other params (like headers, etc) to be passed to $.ajax function
 * @returns {Promise<unknown>}
 */
export function ajaxGet(url: string, data: unknown, miscParams: JQueryAjaxSettings = {}): Promise<unknown> {
    return new Promise(function (resolve, reject) {
        var params = $.extend(
            {
                type: 'GET',
                url: url,
                data: data,
                success: resolve,
                error: ajaxReject(reject)
            },
            miscParams
        );
        $.ajax(params);
    });
}

/**
 * Perform AJAX HEAD request and return promise with status code
 * @param {string} url 
 * @param {unknown} data - data for request
 * @param {Object.<string, unknown>} miscParams - other params (like headers, etc) to be passed to $.ajax function
 * @returns {Promise<number>}
 */
export async function ajaxHead(url: string, data: unknown, miscParams: JQueryAjaxSettings = {}): Promise<number> {
    var params = $.extend(
        {
            type: 'HEAD',
            url: url,
            data: data,
        },
        miscParams
    );
    const ajaxPromise = $.ajax(params);
    return await ajaxPromise.always(
        (xhr: JQueryXHR, _textStatus: string) => xhr.status
    );
}

/**
 * Perform AJAX POST request with CSRF token
 * @param {string} url 
 * @param {unknown} data - data for request
 * @param {Object.<string, string>} extraHeaders - extra headers
 * @param {Object.<string, unknown>} miscParams - other params (like headers, etc) to be passed to $.ajax function
 * @returns {Promise<unknown>}
 */
export function ajaxPost(
    url: string, data: unknown, extraHeaders: { [header: string]: string } = {}, miscParams: JQueryAjaxSettings = {}
): Promise<unknown> {
    return new Promise(function (resolve, reject) {
        var params = $.extend(
            {
                type: 'POST',
                url: url,
                data: data,
                headers: {
                    'X-CSRF-Token': csrfToken,
                    ...extraHeaders
                },
                success: resolve,
                error: ajaxReject(reject)
            },
            miscParams
        );
        $.ajax(params);
    });
}

/**
 * Perform AJAX POST request with CSRF token and sending data in JSON format
 * @param {string} url 
 * @param {unknown} data - data for request
 * @param {Object.<string, string>} extraHeaders - extra headers
 * @param {Object.<string, unknown>} miscParams - other params (like headers, etc) to be passed to $.ajax function
 * @returns {Promise<unknown>}
 */
export function ajaxPostJSON(
    url: string, data: unknown, extraHeaders: { [header: string]: string } = {}, miscParams: JQueryAjaxSettings = {}
): Promise<unknown> {
    return new Promise(function (resolve, reject) {
        var params = $.extend(
            {
                type: 'POST',
                url: url,
                data: JSON.stringify(data),
                contentType: "application/json",
                dataType: "json",
                headers: {
                    'X-CSRF-Token': csrfToken,
                    ...extraHeaders
                },
                success: resolve,
                error: ajaxReject(reject)
            },
            miscParams
        );
        $.ajax(params);
    });
}

/**
 * Perform AJAX PUT request with CSRF token
 * @param {string} url 
 * @param {unknown} data - data for request
 * @param {Object.<string, string>} extraHeaders - extra headers
 * @param {Object.<string, unknown>} miscParams - other params (like headers, etc) to be passed to $.ajax function
 * @returns {Promise<unknown>}
 */
export function ajaxPut(
    url: string, data: unknown, extraHeaders: { [header: string]: string } = {}, miscParams: JQueryAjaxSettings = {}
): Promise<unknown> {
    return new Promise(function (resolve, reject) {
        var params = $.extend(
            {
                type: 'PUT',
                url: url,
                data: data,
                headers: {
                    'X-CSRF-Token': csrfToken,
                    ...extraHeaders
                },
                success: resolve,
                error: ajaxReject(reject)
            },
            miscParams
        );
        $.ajax(params);
    });
}
