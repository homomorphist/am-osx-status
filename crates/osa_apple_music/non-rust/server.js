#!/usr/bin/env osascript -l JavaScript
ObjC.import('AppKit');
ObjC.import('signal');
ObjC.import('stdlib')
ObjC.import('stdio')
ObjC.import('sys/socket')
ObjC.bindFunction('malloc', ['void *', ['int']])
ObjC.bindFunction('memset', ['void *', ['void *', 'int', 'int']])
ObjC.bindFunction('memcpy', ['void *', ['void *', 'void *', 'int']])
ObjC.bindFunction('free', ['void', ['void *']])
ObjC.bindFunction('exit', ['void', ['int']])
ObjC.bindFunction('socket', ['int', ['int', 'int', 'int']])
ObjC.bindFunction('bind', ['int', ['int', 'void *', 'int']])
ObjC.bindFunction('listen', ['int', ['int', 'int']])
ObjC.bindFunction('accept', ['int', ['int', 'void *', 'int *']])
ObjC.bindFunction('read', ['int', ['int', 'void *', 'int']])
ObjC.bindFunction('write', ['int', ['int', 'void *', 'int']])
ObjC.bindFunction('close', ['int', ['int']])
ObjC.bindFunction('perror', ['void', ['void *']])
ObjC.bindFunction('poll', ['int', ['void *', 'int', 'int']])
ObjC.bindFunction('unlink', ['int', ['void *']])
// ObjC.bindFunction('fopen', ['void *', ['void *', 'void *']])
// ObjC.bindFunction('fclose', ['int', ['void *']])
// ObjC.bindFunction('fwrite', ['int', ['void *', 'int', 'int', 'void *']])

const SOCK_STREAM = 1;
const SOCK_DGRAM = 2;
const AF_UNIX = 1;

const POLLIN = 1 << 0;
const POLLOUT = 1 << 2;
const POLLERR = 1 << 3;
const POLLHUP = 1 << 4;

const SIGPIPE = 13;

const SIG_IGN = 1;

$.signal(SIGPIPE, SIG_IGN);

/**
 * @template [T=any]
 * @typedef { number & { ["~"]: "FileDescriptor", ["~T"]: T } } FileDescriptor
 * @typedef { FileDescriptor<"socket"> } Socket
 */
/**
 * @template T
 * @typedef { T | Readonly<T> } MaybeReadonly
 */
//#region Memory Utilities
/**
 * @template T
 * @template { number } N
 * @template { T[] } [A=[]]
 * @typedef { A["length"] extends N ? A : Tuple<T, N, [...A, T]> } Tuple
 */
/**
 * @template [T=any]
 * @typedef { MaybeReadonly<[ptr: Ref<T>, size: number]> } PointerWithSize
 */
/**
 * @typedef { Record<number, number> } ByteIndexable
 */
Ref.prototype.shifted = 0
Ref.prototype.shift = function (offset) {
    const proxied = new Proxy(this, {
        get(target, prop, receiver) {
            if (prop === "__private__") {
                console.log("Attempted to use the internal properties of an offset pointer (via freeing, memcpy, etc).");
                $.exit(1);
            }
            if (prop === "shifted") {
                return target.shifted + offset;
            }
            if (typeof prop === 'string') {
                const numeric = Number(prop);
                if (!isNaN(numeric)) {
                    return target[numeric + offset];
                }
            }
            const descriptor = Object.getOwnPropertyDescriptor(target.constructor.prototype, prop);
            if (descriptor) return descriptor.get.apply(receiver);
            return target[prop];
        },
        set(target, prop, value, receiver) {
            console.log("PROXY_SET", String(prop))
            if (typeof prop === 'string') {
                const numeric = Number(prop);
                if (!isNaN(numeric)) {
                    target[numeric + offset] = value;
                    return true;
                }
            }

            const descriptor = Object.getOwnPropertyDescriptor(target.constructor.prototype, prop);
            if (descriptor) descriptor.set.apply(receiver, [value]);
            else target[prop] = value;
            return true;
        },
    });
    return proxied;
}

/**
 * Allocates a region of memory with the given size and zeros it.
 * Exits with code 1 if allocation fails.
 * @param { number } size - byte count
 * @returns { Ref }
 */
function alloc(size) {
    const ptr = $.malloc(size);
    if (ptr == 0) { 
        $.perror(cstr('Failed to allocate memory'));
        $.exit(1);
    }
    $.memset(ptr, 0, size);
    return ptr;
}

/**
 * @typedef { Ref<"CStr"> } CStrPtr
 * @param { string } str
 * @returns { CStrPtr }
 */
function cstr(str) {
   return cstr.sized(str)[0]
}

/**
 * @param { string } str
 * @returns { PointerWithSize }
 */
cstr.sized = function(str) {
    const utf8 = [];

    for (let i = 0; i < str.length; i++) {
        const code = str.charCodeAt(i);

        if (code < 0x80) {
            utf8.push(code);
        } else if (code < 0x800) {
            utf8.push(0b11000000 | (code >> 6));
            utf8.push(0b10000000 | (code & 0b111111));
        } else if (code >= 0xD800 && code <= 0xDBFF) {
            if (i + 1 >= str.length) {
                // Unpaired high surrogate at end
                utf8.push(0b11101111, 0b10111111, 0b10111101); // U+FFFD
                continue;
            }

            const lo = str.charCodeAt(i + 1);
            if (lo < 0xDC00 || lo > 0xDFFF) {
                // Invalid surrogate pair
                utf8.push(0b11101111, 0b10111111, 0b10111101); // U+FFFD
                continue;
            }

            const cp = 0x10000 + ((code - 0xD800) << 10) + (lo - 0xDC00);
            utf8.push(0b11110000 | (cp >> 18));
            utf8.push(0b10000000 | ((cp >> 12) & 0b111111));
            utf8.push(0b10000000 | ((cp >> 6) & 0b111111));
            utf8.push(0b10000000 | (cp & 0b111111));
            i++; // Advance past the low surrogate
        } else if (code >= 0xDC00 && code <= 0xDFFF) {
            // Unpaired low surrogate
            utf8.push(0b11101111, 0b10111111, 0b10111101); // U+FFFD
        } else {
            utf8.push(0b11100000 | (code >> 12));
            utf8.push(0b10000000 | ((code >> 6) & 0b111111));
            utf8.push(0b10000000 | (code & 0b111111));
        }
    }

    const buf = alloc(utf8.length + 1);
    for (let i = 0; i < utf8.length; i++) {
        buf[i] = utf8[i];
    }
    buf[utf8.length] = 0;

    return /** @type {PointerWithSize} */ ([buf, utf8.length + 1]);
};

/**
 * Decodes a null-terminated UTF-8 C string.
 * Replaces malformed sequences with U+FFFD.
 * @param { ByteIndexable } bytes
 * @returns { string }
 */
function uncstr(bytes) {
    let str = "";
    let i = 0;

    while (bytes[i] !== 0) {
        const byte1 = bytes[i++];
        if (byte1 === undefined) {
            str += "\uFFFD";
            break;
        }

        if (byte1 < 0x80) {
            str += String.fromCharCode(byte1);
        } else if (byte1 < 0xE0) {
            const byte2 = bytes[i++];
            if ((byte2 & 0xC0) !== 0x80) {
                str += "\uFFFD";
                if (byte2 !== undefined) i--; // step back if not EOF
                continue;
            }
            str += String.fromCharCode(
                ((byte1 & 0b11111) << 6) |
                 (byte2 & 0b111111)
            );
        } else if (byte1 < 0xF0) {
            const byte2 = bytes[i++];
            const byte3 = bytes[i++];
            if (
                (byte2 & 0xC0) !== 0x80 ||
                (byte3 & 0xC0) !== 0x80
            ) {
                str += "\uFFFD";
                if (byte2 !== undefined) i--;
                if (byte3 !== undefined) i--;
                continue;
            }
            str += String.fromCharCode(
                ((byte1 & 0b1111) << 12) |
                ((byte2 & 0b111111) << 6) |
                 (byte3 & 0b111111)
            );
        } else if (byte1 < 0xF8) {
            const byte2 = bytes[i++];
            const byte3 = bytes[i++];
            const byte4 = bytes[i++];
            if (
                (byte2 & 0xC0) !== 0x80 ||
                (byte3 & 0xC0) !== 0x80 ||
                (byte4 & 0xC0) !== 0x80
            ) {
                str += "\uFFFD";
                if (byte2 !== undefined) i--;
                if (byte3 !== undefined) i--;
                if (byte4 !== undefined) i--;
                continue;
            }

            const cp =
                (((byte1 & 0b111) << 18) |
                 ((byte2 & 0b111111) << 12) |
                 ((byte3 & 0b111111) << 6) |
                  (byte4 & 0b111111)) - 0x10000;

            if (cp < 0 || cp > 0x10FFFF) {
                str += "\uFFFD";
                continue;
            }

            str += String.fromCharCode(
                0xD800 + (cp >> 10),
                0xDC00 + (cp & 0x3FF)
            );
        } else {
            str += "\uFFFD";
        }
    }

    return str;
}

/**
 * Returns an array of the given size with the contents of the memory at the given pointer.
 * @param { Ref } ptr 
 * @param { number } size 
 * @returns { Tuple<number, 4> }
 */
function arr(ptr, size) {
    const arr = new Array(size);
    for (let i = 0; i < size; i++) {
        arr[i] = ptr[i];
    }
    return /** @type { Tuple<number, 4> } */ (arr);
}

/**
 * Memory copying utility; since `memcpy` isn't super useful because of the lack of non-hacky pointer arithmetic.
 * 
 * @typedef { Ref | [ptr: Ref, plus: number] } PointerMaybeOffset
 * 
 * @type { (
 *  ((from: PointerMaybeOffset, to: ByteIndexable | PointerMaybeOffset, bytes: number) => void) &
 *  ((from: ByteIndexable, to: ByteIndexable | PointerMaybeOffset, bytes?: number) => void)
 * )}
 */
const cpy = (from, to, bytes) => {
    let from_indexable
    let from_offset;
    let to_indexable;
    let to_offset;
    let all_ptr_no_offset = true;
    if (typeof from[0] === 'function') { // is ptr + offset
        from_indexable = from[0];
        from_offset = from[1];
        all_ptr_no_offset &&= from_offset === 0;
    } else {
        all_ptr_no_offset &&= typeof from === 'function' && from.shifted === 0;
        from_indexable = from;
        from_offset = 0;
    }
    if (typeof to[0] === 'function') { // is ptr + offset
        to_indexable = to[0];
        to_offset = to[1];
        all_ptr_no_offset &&= to_offset === 0;
    } else {
        all_ptr_no_offset &&= typeof to === 'function' && to.shifted === 0;
        to_indexable = to;
        to_offset = 0;
    }

    if (all_ptr_no_offset) {
        if (bytes === undefined) {
            console.log("Missing required byte transfer count.")
            $.exit(1);
        }
        $.memcpy(to_indexable, from_indexable, bytes);
    } else {
        
        if (from_indexable instanceof Array) {
            if (bytes && to_indexable instanceof Array && to.length < bytes) {
                console.log("Destination array has insufficient length (wanted to copy " + bytes + " bytes, but only " + to_indexable.length + " were available).");
                $.exit(1);
            } else if (bytes === undefined) {
                bytes = from_indexable.length;
            }

            if (bytes > from_indexable.length) {
                console.log("Source array has insufficient length (wanted to copy " + bytes + " bytes, but only " + from_indexable.length + " were available).");
                $.exit(1);
            }
        }

        for (let i = 0; i < bytes; i++) {
            to_indexable[to_offset + i] = from_indexable[from_offset + i];
        }
    }
}



/**
 * Conversion namespace.
 */
const conv = (() => {
    const conv = {};

    /**
     * Converts a 32-bit integer to an array of 4 bytes.
     * @param { number } int32
     * @returns { Tuple<number, 4> }
     */
    conv.i32_to_a4u8 = (int32) => {
        const a = int32 & 0xFF;
        const b = (int32 >> 8) & 0xFF;
        const c = (int32 >> 16) & 0xFF;
        const d = (int32 >> 24) & 0xFF;
        return [a, b, c, d];
    }

    /**
     * Converts four bytes to a 32-bit integer.
     * @type { |
     *  ((tuple: Tuple<number, 4>) => number) &
     *  ((pointer: Ref, offset?: number) => number)
     * }
     */
    conv.a4u8_to_i32 = (bytes, offset = 0) => bytes[offset] |
        (bytes[offset + 1] << 8) |
        (bytes[offset + 2] << 16) |
        (bytes[offset + 3] << 24);
    
    /**
     * Converts a 16-bit integer to an array of 2 bytes.
     * @param { number } int16
     * @returns { Tuple<number, 2> }
     */
    conv.i16_to_2u8 = (int16) => [
        (int16 & 0xFF),
        ((int16 >> 8) & 0xFF)
    ]

    /**
     * Converts two bytes to a 32-bit integer.
     * @type { |
     *  ((tuple: Tuple<number, 2>) => number) &
     *  ((pointer: Ref, offset?: number) => number)
     * }
     */
    conv.a2u8_to_int16 = (bytes, offset = 0) => bytes[offset] | (bytes[offset + 1] << 8)

    return conv
})()

//#endregion
//#region Structs
sockaddr_un.sizeof = 106;
/**
 * @typedef { Ref<"sockaddr_un"> } sockaddr_un
 * @param { string } path
 * @returns { sockaddr_un }
 */
function sockaddr_un(path) {
    if (path.length > sockaddr_un.sizeof - 2) { console.log('Socket path is too long'); $.exit(1); }
    const buf = alloc(sockaddr_un.sizeof);
    buf[0] = sockaddr_un.sizeof
    buf[1] = AF_UNIX; 
    for (let i = 0; i < path.length; i++) {
        buf[2 + i] = path.charCodeAt(i);
    }
    return /** @type { sockaddr_un } */ (buf);
}


/**
 * @template T
 * @typedef { (T extends any ? (x: T) =>void : never) extends ((x: infer U) => void) ? U : never } UnionToIntersection
 */
/**
 * @template T
 * @typedef { {} & { [K in keyof T]: T[K] } } Remap
 */
/**
 * @template { string } [N=any]
 * @template { StructLayoutFieldDefinition<N>[] } [L=[]]
 * @typedef { Ref<N> & Remap<UnionToIntersection<
 *  { [N in keyof L]: "set" extends keyof L[N][3] 
 *    ? { [K in L[N][0]]: ReturnType<L[N][3]["get"]> }
 *    : { readonly [K in L[N][0]]: ReturnType<L[N][3]["get"]> }
 *   }[number]>>
 * } StructInstance
 */
/**
 * @template { string } [N=any]
 * @template { StructLayoutFieldDefinition<N>[] } [L=[]]
 * @template { any[] } [A=any[]]
 * @typedef { {
 *   name: N,
 *   sizeof: number,
 *   alloc_null: () => Ref<N>,
 *   from_ptr: (ptr: Ref<N>) => StructInstance<N, L>,
 * } } StructConstructorStatic
 */
/**
 * @template { string } [N=any]
 * @template { StructLayoutFieldDefinition<N>[] } [L=[]]
 * @template { any[] } [A=any[]]
 * @typedef { (new (...args: A) => StructInstance<N, L>) & StructConstructorStatic<N, L, A> } StructConstructor
 */
/**
 * @template { string } N
 * @template T
 * @typedef {{
 *    get?: (ptr: Ref<N>, offset: number) => T
 *    set?: (ptr: Ref<N>, offset: number, value: T) => void
 * }} StructLayoutFieldDefinitionAccessors
 */
/**
 * @template { string } N
 * @template { string } [PN=string]
 * @template [T=any]
 * @typedef { [name: PN, type: string, bytes: number, StructLayoutFieldDefinitionAccessors<N, T>] } StructLayoutFieldDefinition
 */
/**
 * @template { string } const N
 * @template { StructLayoutFieldDefinition<N>[] } const L
 * @template { any[] } const A=[]
 * @param { N } name
 * @param { L } members
 * @param { (this: { ptr: Ref<N> } & StructInstance<N, L>, ...args: A) => void } [init]
 */
function define_struct(name, members, init) {
    const [accessors, sizeof] = members.reduce(([acc, offset], [property,,len, desc]) => /** @type { [PropertyDescriptorMap, number] } */([{
        ...acc,
        [property]: {
            get: desc.get
                ? function () {return desc.get(this, offset); }
                : function () { throw new Error(`Unimplemented getter for property \"${property}\" of struct \"${name}\"`); },
            set: desc.set
                ? function (value) { return desc.set(this, offset, value) }
                : function () { throw new Error(`Unimplemented setter for property \"${property}\" of struct \"${name}\"`); },
        }
    }, /* Padding? What's padding? */ offset + len]), /** @type { [PropertyDescriptorMap, number] } */([{}, 0]));

    /**
     * @type { StructConstructor<N, L, A> }
     */
    // @ts-ignore
    const klass = class {
        static sizeof = sizeof;
        static from_ptr(ptr) {
            Reflect.setPrototypeOf(ptr, klass.prototype);
            return ptr;
        }
        static alloc_null() {
            return alloc(this.sizeof);
        }

        /**
         * @param { A } args
         */
        constructor(...args) {
            init?.call(this, ...args);
        }
    };

    klass.prototype.__proto__ = Ref.prototype;
    Object.defineProperty(klass, "name", { value: name });
    Object.defineProperties(klass.prototype, accessors)
    const tag = `Struct<${JSON.stringify(name)}>`;
    klass[Symbol.toStringTag] = tag;

    /**
     * @type { StructConstructor<N, L, A> }
     */
    return new Proxy(klass, {
        get(target, prop, receiver) {
            if (prop === "toString") {
                return () => tag
            }
            return Reflect.get(target, prop, receiver);
        },
        construct(target, args) {
            const alloc = target.alloc_null(); 
            Reflect.setPrototypeOf(alloc, klass.prototype);
            init?.call(alloc, ...args);
            return alloc;
        }
    });
}

/**
 * Shorthand struct member type definitions.
 */
const t = (() => {
    const t = {};

    /**
     * @param { number } bytes
     * @returns { StructLayoutFieldDefinitionAccessors<never, number> }
     */
    function mk_int_accessors(bytes) {
        let from;
        let to;

        switch (bytes) {
            case 4:
                from = conv.a4u8_to_i32;
                to = conv.i32_to_a4u8;
                break;
            case 2:
                from = conv.a2u8_to_int16;
                to = conv.i16_to_2u8;
                break;
            default:
                throw new Error(`Unsupported size: ${bytes}`);
        }

        return {
            get: (a, offset) => from(a, offset),
            set: (a, offset, value) => cpy(to(value), [a, offset])
        }
    }

    /**
     * @template const T
     * @param { T } v
     * @returns { T }
     */
    function c(v) {
        return v
    }

    /**
     * @param { string } type
     * @param { number } bytes
     */
    function def_int(type, bytes) {
        return c([type, bytes, mk_int_accessors(bytes)]);
    }

    /**
     * @type { StructLayoutFieldDefinitionAccessors<never, FileDescriptor> }
     */
    // @ts-expect-error :: Unsafe cast.
    const int4fd_accessors = mk_int_accessors(4);

    t.int4fd = c(["int fd", 4, int4fd_accessors]);
    t.int4 = def_int("int", 4);
    t.int2 = def_int("short", 2)

    return t;
})()

/**
 * @typedef { InstanceType<typeof pollfd> } pollfd
 */
const pollfd = define_struct("pollfd",
    [
        ["fd", ...t.int4fd],
        ["events", ...t.int2],
        ["revents", ...t.int2]
    ],
    /**
     * @param { FileDescriptor } fd 
     * @param { number } events - bitmask of events to poll for
     */
    function (fd, events) {
        this.fd = fd;
        this.events = events;
    }
)
//#endregion

/**
 * @template { StructConstructor } T
 */
class AllocatedStructArray {
    /**
     * @type { T }
     */
    struct;

    /**
     * @param { T } struct 
     * @param { number } capacity 
     */
    constructor (struct, capacity) { 
        this.capacity = capacity;
        this.struct = struct;
        this.length = 0;
        this.ptr = alloc(struct.sizeof * capacity);
    }

    /**
     * @param { number } index
     * @param { { copy?: boolean | undefined; } } options - whether to return a copy of the value or a live pointer to the value
     * @return { InstanceType<T> | null } 
     */
    get(index, { copy }) {
        if (index >= this.length) { console.log("Out of bounds read (idx =", index, ", len = ", this.length, ")"); return null }
        if (index < 0) { console.log("Out of bounds read (idx =", index, ")"); return null }
        const offset = index * this.struct.sizeof;
        let ptr;

        if (copy) {
            const buf = this.struct.alloc_null();
            cpy([this.ptr, offset], buf, this.struct.sizeof);
            ptr = buf;
        } else {
            const offset = index * this.struct.sizeof;
            ptr = this.ptr.shift(offset);
        }

        //@ts-expect-error :: TODO: Fix typings.
        return this.struct.from_ptr(ptr);
    }

    /**
     * @param { number } index
     * @param { InstanceType<T> } val - pointer to the value to copy and write 
     * @returns { boolean } - success
     * @private (to prevent array holes)
     */
    set(index, val) {
        if (index >= this.capacity) return false ;
        if (index < 0) return false;
        cpy(val, [this.ptr, index * this.struct.sizeof], this.struct.sizeof);
        return true;
    }

    /**
     * @param { InstanceType<T> } val - value to copy and write
     * @param { { free_original?: boolean | undefined; } } options
     * @returns { boolean } - success
     */
    push(val, { free_original }) {
        if (this.length >= this.capacity) return false;
        this.set(this.length++, val);
        if (free_original) $.free(val);
        return true
    }

    /**
     * @param { { copy?: boolean | undefined; } } [options]
     * @returns { InstanceType<T> | null } a pointer to the (the potentially uncopied) last value, which is removed from the array (but not immediately zero'd)
     */
    pop({ copy } = { copy: true }) {
        if (this.length == 0) return null;
        const got = this.get(this.length - 1, { copy });
        this.length--;
        return got
    }

    /**
     * @param { { copy?: boolean | undefined; } } options
     * @returns { InstanceType<T> | null } a pointer to (the potentially copied) first value
     */
    first({ copy }) {
        return this.get(0, { copy });
    }

    /**
     * @param { number } index 
     */
    // it's Beautiful.
    remove(index) {
        const right = this.length - index - 1;
        const temp = new AllocatedStructArray(this.struct, right);
        for (let i = 0; i < right; i++) {
            temp.push(this.pop({ copy: false }), { free_original: false });
        }
        this.pop();
        for (let i = 0; i < right; i++) {
            this.push(temp.pop({ copy: false }), { free_original: false });
        }
        temp.free();
    }

    /**
     * @param { (value: InstanceType<T>, index: number) => boolean } callback
     * @return { number } index of found value, or -1
     */
    findIndex(callback) {
        for (let i = 0; i < this.length; i++) {
            const value = this.get(i, { copy: true });
            const matches = callback(value, i);
            if (matches) return i;
        }
        return -1;
    }

    free() {
        $.free(this.ptr);
    }

    /**
     * Returns an iterator over every element; returns a live pointer to each element.
     * Mutation while iterating is undefined behavior.
     * @returns { Iterator<InstanceType<T>> }
     */
    [Symbol.iterator]() {
        let i = 0;
        return {
            next: () => {
                if (i >= this.length) return { done: true, value: undefined };
                const value = this.get(i++, { copy: false });
                return { value, done: !value }
            }
        }
    }
}

class ClientConnection {
    /**
     * @param { pollfd } pfd
     * @param { Server } server
     */
    constructor(pfd, server) {
        this.pollfd = pfd;
        this.server = server;
    }

    get fd() {
        return this.pollfd.fd;
    }

    close() {
        this.server.close(this.pollfd);
    }

    /**
     * @param { PointerWithSize } data
     * @returns { number } the amount of bytes written
     */
    write([ptr, size]) {
        return $.write(this.pollfd.fd, ptr, size)
    }

    /**
     * @param { PointerWithSize } data
     * @returns { number } the amount of bytes written (the pointer size)
     */
    write_all([ptr, size]) {
        let written = 0;
        while (written < size) {
            const result = this.write([ptr, size - written]);
            if (result <= 0) break;
            written += result;
            ptr = ptr.shift(written);
        }
        return written;
    }
}

class Server {
    /**
     * Creates a streaming unix socket that can communicate with multiple clients via polling.
     * Errors on socket creation failure.
     * @param { string } path - path to socket
     * @param {{
     *  max_clients: number,
     *  backlog: number
     * } } options
     */
    constructor(path, options) {
        this.backlog = options.backlog;
        this.max_clients = options.max_clients;
        $.unlink(cstr(path)); // failure OK

        const socket_fd = $.socket(AF_UNIX, SOCK_STREAM, 0);
        if (socket_fd < 0) {
            $.perror(cstr('Failed to create socket'));
            $.exit(1);
        }
        
        const addr = sockaddr_un(path);
        const bind = $.bind(socket_fd, addr, sockaddr_un.sizeof);
        $.free(addr);
        if (bind < 0) { 
            $.perror(cstr('Failed to bind socket'));
            $.exit(1);
        }

        const listener = new pollfd(socket_fd, POLLIN);
        this.watching = new AllocatedStructArray(pollfd, this.max_clients);
        this.watching.push(listener, { free_original: true });
        this.listener = pollfd.from_ptr(this.watching.ptr)
    }

    /**
     * @private
     */
    accept_new_client() {
        const client = $.accept(this.listener.fd, 0, 0);
        if (client < 0) {
            $.perror(cstr('Failed to accept connection'));
            $.exit(1);
        }
        this.watching.push(new pollfd(client, POLLIN), { free_original: true });
    }

    /**
     * Closes the connection with the given client.
     * Does not free the `pollfd` struct.
     * @param { pollfd } client
     */
    close(client) {
        const fd = client.fd;
        $.close(fd);
        const index = this.watching.findIndex(connected => connected.fd === fd);
        if (index === -1) {
            console.log(client.fd, "isn't in the list of connected clients, so it can't be closed.");
            return;
        }
        if (index === 0) {
            console.log("Tried to remove index the listener from the watched file descriptors.");
            $.exit(1);
        }
        this.watching.remove(index);
    }
    
    /**
     * @returns { IterableIterator<pollfd> }
     */
    clients() {
        const iterator = this.watching[Symbol.iterator]();
        iterator[Symbol.iterator] = () => iterator;
        iterator.next(); // skip listener
        //@ts-ignore :: We set it.
        return iterator;
    }

    /**
     * Start the server and listen for connections.
     * @param { (connection: ClientConnection, data: PointerWithSize) => void } callback
     */
    listen(callback) {
        if ($.listen(this.listener.fd, this.backlog) < 0) {
            $.perror(cstr('listen() failed'));
            $.exit(1);
        }
        console.log("Listening for connections...");


        while (true) {
            const ready = $.poll(this.watching.ptr, this.watching.length, -1);
            if (ready < 0) {
                $.perror(cstr('poll() failure'));
                $.exit(1);
            }

            if (this.listener.revents & POLLIN) {
                this.accept_new_client();
            }
            
            const clients_to_close = [];

            for (const client of this.clients()) {
                const received_events = client.revents;

                if (received_events & POLLIN) {
                    // TODO: Read-loop.
                    const buffer_size = 1024;
                    const buffer = alloc(buffer_size);
                    const size = $.read(client.fd, buffer, buffer_size);
                    if (size > 0) {
                        const wrapped = new ClientConnection(client, this);
                        callback(wrapped, [buffer, size]);
                    }
                    $.free(buffer);
                    if (size < 0) {
                        $.perror(cstr('Failed to read from socket ' +  client.fd));
                        $.exit(1);
                    }
                }

                if (received_events & (POLLHUP | POLLERR)) {
                    clients_to_close.push(client);
                }
            }

            for (const client of clients_to_close) {
                this.close(client);
            }
        }
    }
}

/**
 * @param { Error } err 
 * @returns { object }
 */
function copy_err_to_plain_object(err) {
    const plain = Object.create(null);
    for (const key of Reflect.ownKeys(err)) {
        plain[key] = err[key];
    }
    return plain
}

const prelude = ["osascript", "-l", "JavaScript", "<script-file>"];
const usage = [...prelude, "<socket-path>"]
const args = $.NSProcessInfo.processInfo.arguments.js.slice(prelude.length).map(arg => arg.js)
const path = args[0];
if (!path) {
    console.log("usage: " + usage.join(" "));
    $.exit(1);
}

const server = new Server(path, {
    max_clients: 3,
    backlog: 5
});

const APPLE_MUSIC = "com.apple.Music";

server.listen((connection, [data]) => {
    /**
     * @type { PointerWithSize }
     */
    let str;
    try {
        const app = Application(APPLE_MUSIC);
        if (!app.running()) throw new Error("Application not running");

        let output;
        switch (uncstr(data).trim()) {
            case "application":   { output = app             .properties(); break }
            case "current track": { output = app.currentTrack.properties(); break }
            default: throw new Error("Unknown command");
        }

        str = cstr.sized(JSON.stringify({
            type: "success",
            value: output
        }));
    } catch (err) {
        str = cstr.sized(JSON.stringify({
            type: "error",
            value: copy_err_to_plain_object(err)
        }))
    }
    connection.write_all(str);
    $.free(str[0]);
})
