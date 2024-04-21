/// <amd-module name='archivanima/post_add'/>

import { uploadFile, addPost, editPost } from 'archivanima/api';
import { unwrapEitherOrThrow } from 'archivanima/utils';

export class PostAddForm {
    form: HTMLFormElement;
    button: HTMLButtonElement;
    titleField: HTMLInputElement;
    descriptionField: HTMLInputElement;
    hiddenField: HTMLInputElement;
    pinnedField: HTMLInputElement;
    minAgeField: HTMLInputElement;
    fileField: HTMLInputElement;
    progressCell: HTMLElement;
    chunkSize: number;

    constructor(form: HTMLFormElement, chunkSize: number) {
        this.form = form;
        this.button = <HTMLButtonElement>form.querySelector('button#button-upload');
        this.titleField = <HTMLInputElement>form.querySelector('input#input-title');
        this.pinnedField = <HTMLInputElement>form.querySelector('input#input-pinned');
        this.descriptionField = <HTMLInputElement>form.querySelector('textarea#input-description');
        this.hiddenField = <HTMLInputElement>form.querySelector('input#input-hidden');
        this.minAgeField = <HTMLInputElement>form.querySelector('input#input-min_age');
        this.fileField = <HTMLInputElement>form.querySelector('input#input-file');
        this.progressCell = <HTMLElement>form.querySelector('#cell-progress');
        this.chunkSize = chunkSize;

        this.form.addEventListener('submit', (event: Event) => this.onFormSubmit(event));
        this.button.disabled = false;
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
        this.pinnedField.disabled = true;
        this.fileField.disabled = true;
        this.button.disabled = true;

        const title = this.titleField.value;
        const description = this.descriptionField.value;
        const isHidden = this.hiddenField.checked;
        const isPinned = this.pinnedField.checked;
        const minAge = this.minAgeField.valueAsNumber;
        const mustHideAndUnhide = !isHidden && (this.fileField.files.length > 0);

        const postResult = unwrapEitherOrThrow(await addPost(
            title, description, mustHideAndUnhide ? true : isHidden, isPinned,
            Number.isNaN(minAge) ? null : minAge
        ));

        const files = Array.from(this.fileField.files);
        for (let id = 0; id < files.length; id++) {
            const file = files[id];
            const progressBar = this.addProgressBar(file.name, id);
            unwrapEitherOrThrow(await uploadFile(
                file, this.chunkSize, postResult.id,
                (id, uploadedSize, totalSize) => {
                    console.log(`Upload ID ${id}, progress ${uploadedSize} / ${totalSize}`);
                    progressBar.value = uploadedSize;
                    progressBar.max = totalSize;
                }
            ));
            // TODO: print errors and result
        }

        if (mustHideAndUnhide) {
            unwrapEitherOrThrow(await editPost(
                postResult.id, title, description, false, isPinned,
                Number.isNaN(minAge) ? null : minAge
            ));
        }

        document.location.assign(postResult.url);
    }
}
