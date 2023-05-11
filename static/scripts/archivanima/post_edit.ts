/// <amd-module name='archivanima/post_edit'/>

import { uploadFile, removeFile, editPost } from 'archivanima/api';

export class PostEditForm {
    id: number;
    url: string;
    form: HTMLFormElement;
    button: HTMLButtonElement;
    titleField: HTMLInputElement;
    descriptionField: HTMLInputElement;
    hiddenField: HTMLInputElement;
    minAgeField: HTMLInputElement;
    fileField: HTMLInputElement;
    progressCell: HTMLElement;
    uploadItemElements: HTMLElement[];
    chunkSize: number;

    removedFiles: Set<number>;

    constructor(form: HTMLFormElement, chunkSize: number) {
        this.form = form;
        this.button = <HTMLButtonElement>form.querySelector('button#button-upload');
        this.titleField = <HTMLInputElement>form.querySelector('input#input-title');
        this.descriptionField = <HTMLInputElement>form.querySelector('textarea#input-description');
        this.hiddenField = <HTMLInputElement>form.querySelector('input#input-hidden');
        this.minAgeField = <HTMLInputElement>form.querySelector('input#input-min_age');
        this.fileField = <HTMLInputElement>form.querySelector('input#input-file');
        this.progressCell = <HTMLElement>form.querySelector('#cell-progress');
        this.uploadItemElements = Array.from(form.querySelectorAll('.upload-item'));
        this.chunkSize = chunkSize;
        this.id = Number.parseInt(<string>form.dataset.id);
        this.url = <string>form.dataset.url;

        this.removedFiles = new Set();

        this.form.addEventListener('submit', (event: Event) => this.onFormSubmit(event));

        for (let uploadItemElement of this.uploadItemElements) {
            const id = Number.parseInt(<string>uploadItemElement.dataset.id);
            const uploadLink = <HTMLElement>uploadItemElement.querySelector('.upload-item-link');
            const toggleElement = <HTMLLinkElement>uploadItemElement.querySelector('a.upload-item-toggle');
            toggleElement.addEventListener('click', (event: Event) => this.onUploadItemToggle(event, id, toggleElement, uploadLink));
        }

        this.button.disabled = false;
    }

    private async onUploadItemToggle(event: Event, id: number, toggleElement: HTMLElement, uploadItemElement: HTMLElement) {
        event.preventDefault();
        if (this.removedFiles.has(id)) {
            toggleElement.textContent = 'удалить';
            uploadItemElement.classList.remove('item-removed');
            this.removedFiles.delete(id);
        } else {
            toggleElement.textContent = 'восстановить';
            uploadItemElement.classList.add('item-removed');
            this.removedFiles.add(id);
        }
    }

    private addProgressBar(fileName: string, id: number): HTMLProgressElement {
        const progressElementId = `progress-file-${id}`;

        const labelElement = document.createElement('label');
        labelElement.textContent = fileName;
        labelElement.setAttribute('for', progressElementId);

        const progressElement = document.createElement('progress');
        progressElement.value = 0;
        progressElement.max = 1;
        progressElement.id = progressElementId;

        this.progressCell.appendChild(progressElement);
        this.progressCell.appendChild(labelElement);
        this.progressCell.appendChild(document.createElement('br'));

        return progressElement;
    }

    private async onFormSubmit(event: Event) {
        event.preventDefault();
        if (this.fileField.files === null) {
            return;
        }

        this.titleField.disabled = true;
        this.descriptionField.disabled = true;
        this.hiddenField.disabled = true;
        this.fileField.disabled = true;
        this.button.disabled = true;

        const title = this.titleField.value;
        const description = this.descriptionField.value;
        const isHidden = this.hiddenField.checked;
        const minAge = this.minAgeField.valueAsNumber;

        await editPost(
            this.id, title, description, isHidden,
            Number.isNaN(minAge) ? null : minAge,
            (xhr: JQueryXHR, textStatus: string, errorThrown: string) => {
                // TODO
                console.error(`Error: ${xhr}, ${textStatus}, ${errorThrown}`);
            }
        );

        for (let fileId of Array.from(this.removedFiles)) {
            await removeFile(
                fileId,
                (xhr: JQueryXHR, textStatus: string, errorThrown: string) => {
                    // TODO
                    console.error(`Error: ${xhr}, ${textStatus}, ${errorThrown}`);
                }
            );
        }

        const files = Array.from(this.fileField.files);
        for (let id = 0; id < files.length; id++) {
            const file = files[id];
            const progressBar = this.addProgressBar(file.name, id);
            await uploadFile(
                file, this.chunkSize, this.id,
                (id, uploadedSize, totalSize) => {
                    console.log(`Upload ID ${id}, progress ${uploadedSize} / ${totalSize}`);
                    progressBar.value = uploadedSize;
                    progressBar.max = totalSize;
                },
                (xhr: JQueryXHR, textStatus: string, errorThrown: string) => {
                    // TODO
                    console.error(`Error: ${xhr}, ${textStatus}, ${errorThrown}`);
                }
            );
            // TODO: print errors and result
        }

        document.location.assign(this.url);
    }
}
