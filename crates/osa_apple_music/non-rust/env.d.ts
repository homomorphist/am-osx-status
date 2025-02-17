declare const $: any;

declare const ObjC: {
    import(framework: string): any;
    bindFunction(name: string, ffi: [output: string, arguments: Array<string>]): void;
}
declare function Application(name: string): any;

/**
 * A pointer to a location in memory.
 * Raw bytes can be accessed by indexing into the pointer.
 */
declare interface Ref<T = any> {
    ["~"]: "Ref";
    ["~T"]: T;

    [index: number]: number;

    /**
     * @returns a proxy that will read and write the bytes at this pointer but with an added offset
     */
    // Not native; implementation is in index.ts
    shift(offset: number): this;

    /**
     * The level of shifting applied on this layer of proxying. Set to zero for non-proxied.
     */
    // Not native; implementation is in index.ts
    shifted: number;
}
declare function Ref<T>(value: T): Ref<T>;
