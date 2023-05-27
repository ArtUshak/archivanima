/// <amd-module name='archivanima/utils'/>

class Some<A> {
    value: A;
    constructor(value: A) {
        this.value = value;
    }
}

class None {
    constructor() { }
}

export type Option<A> = Some<A> | None;

export function some<A>(a: A): Some<A> {
    return new Some(a);
}

export function none(): None {
    return new None;
}

export function isSome<A>(option: Option<A>): boolean {
    return option instanceof Some;
}

export function isNone<A>(option: Option<A>): boolean {
    return option instanceof None;
}

export function map<A, B>(option: Option<A>, func: (value: A) => B): Option<B> {
    if (option instanceof Some) {
        return new Some(func((<Some<A>>option).value));
    } else {
        return new None();
    }
}

export function unwrapOrElse<A>(option: Option<A>, defaultValueProducer: () => A): A {
    if (option instanceof Some) {
        return (<Some<A>>option).value;
    } else {
        return defaultValueProducer();
    }
}

export function unwrapOr<A>(option: Option<A>, defaultValue: A): A {
    return unwrapOrElse(option, () => defaultValue);
}

export function unwrapOrUndefined<A>(option: Option<A>): A | undefined {
    if (option instanceof Some) {
        return (<Some<A>>option).value;
    } else {
        return undefined;
    }
}

export function unwrapOrNull<A>(option: Option<A>): A | null {
    if (option instanceof Some) {
        return (<Some<A>>option).value;
    } else {
        return null;
    }
}

export function unwrapOrThrow<A>(option: Option<A>): A {
    if (option instanceof Some) {
        return (<Some<A>>option).value;
    } else {
        throw null;
    }
}

class Left<A> {
    value: A;
    constructor(value: A) {
        this.value = value;
    }
}

class Right<B> {
    value: B;
    constructor(value: B) {
        this.value = value;
    }
}

export type Either<A, B> = (Left<A> | Right<B>);

export function left<A>(a: A): Left<A> {
    return new Left(a);
}

export function right<B>(a: B): Right<B> {
    return new Right(a);
}

export function isLeft<A, B>(either: Either<A, B>): boolean {
    return either instanceof Left;
}

export function isRight<A, B>(either: Either<A, B>): boolean {
    return either instanceof Right;
}

export function mapLeft<A, B, Y>(either: Either<A, B>, func: (value: A) => Y): Either<Y, B> {
    if (either instanceof Left) {
        return new Left(func((<Left<A>>either).value));
    } else {
        return either;
    }
}

export function mapRight<A, B, Y>(either: Either<A, B>, func: (value: B) => Y): Either<A, Y> {
    if (either instanceof Left) {
        return either;
    } else {
        return new Right(func((<Right<B>>either).value));
    }
}

export function getLeft<A, B>(either: Either<A, B>): Option<A> {
    if (either instanceof Left) {
        return some((<Left<A>>either).value);
    } else {
        return none();
    }
}

export function getRight<A, B>(either: Either<A, B>): Option<B> {
    if (either instanceof Left) {
        return none();
    } else {
        return some((<Right<B>>either).value);
    }
}

export function combine<A, B>(either: Either<A, Either<A, B>>): Either<A, B> {
    if (either instanceof Left) {
        return either;
    } else {
        return (<Right<Either<A, B>>>either).value;
    }
}

export function unwrapEitherOrThrow<A, B>(either: Either<A, B>): A {
    if (either instanceof Left) {
        return (<Left<A>>either).value;
    } else {
        throw (<Right<B>>either).value;
    }
}
