// This file was autogenerated by some hot garbage in the `uniffi-bindgen-react-native` crate.
// Trust me, you don't want to mess with it!
import nativeModule, {
  type UniffiRustFutureContinuationCallback,
  type UniffiForeignFutureFree,
  type UniffiCallbackInterfaceFree,
  type UniffiForeignFuture,
  type UniffiForeignFutureStructU8,
  type UniffiForeignFutureCompleteU8,
  type UniffiForeignFutureStructI8,
  type UniffiForeignFutureCompleteI8,
  type UniffiForeignFutureStructU16,
  type UniffiForeignFutureCompleteU16,
  type UniffiForeignFutureStructI16,
  type UniffiForeignFutureCompleteI16,
  type UniffiForeignFutureStructU32,
  type UniffiForeignFutureCompleteU32,
  type UniffiForeignFutureStructI32,
  type UniffiForeignFutureCompleteI32,
  type UniffiForeignFutureStructU64,
  type UniffiForeignFutureCompleteU64,
  type UniffiForeignFutureStructI64,
  type UniffiForeignFutureCompleteI64,
  type UniffiForeignFutureStructF32,
  type UniffiForeignFutureCompleteF32,
  type UniffiForeignFutureStructF64,
  type UniffiForeignFutureCompleteF64,
  type UniffiForeignFutureStructPointer,
  type UniffiForeignFutureCompletePointer,
  type UniffiForeignFutureStructRustBuffer,
  type UniffiForeignFutureCompleteRustBuffer,
  type UniffiForeignFutureStructVoid,
  type UniffiForeignFutureCompleteVoid,
} from './lni-ffi';
import {
  type FfiConverter,
  type UniffiObjectFactory,
  type UniffiRustArcPtr,
  type UnsafeMutableRawPointer,
  AbstractFfiConverterArrayBuffer,
  FfiConverterInt32,
  FfiConverterObject,
  FfiConverterUInt64,
  RustBuffer,
  UniffiAbstractObject,
  UniffiError,
  UniffiInternalError,
  destructorGuardSymbol,
  pointerLiteralSymbol,
  rustCall,
  uniffiCreateRecord,
  uniffiRustCallAsync,
  uniffiTypeNameSymbol,
  variantOrdinalSymbol,
} from 'uniffi-bindgen-react-native';

// Get converters from the other files, if any.

const uniffiIsDebug =
  // @ts-ignore -- The process global might not be defined
  typeof process !== 'object' ||
  // @ts-ignore -- The process global might not be defined
  process?.env?.NODE_ENV !== 'production' ||
  false;
// Public interface members begin here.

export type Ip = {
  origin: string;
};

/**
 * Generated factory for {@link Ip} record objects.
 */
export const Ip = (() => {
  const defaults = () => ({});
  const create = (() => {
    return uniffiCreateRecord<Ip, ReturnType<typeof defaults>>(defaults);
  })();
  return Object.freeze({
    /**
     * Create a frozen instance of {@link Ip}, with defaults specified
     * in Rust, in the {@link lni} crate.
     */
    create,

    /**
     * Create a frozen instance of {@link Ip}, with defaults specified
     * in Rust, in the {@link lni} crate.
     */
    new: create,

    /**
     * Defaults specified in the {@link lni} crate.
     */
    defaults: () => Object.freeze(defaults()) as Partial<Ip>,
  });
})();

const FfiConverterTypeIp = (() => {
  type TypeName = Ip;
  class FFIConverter extends AbstractFfiConverterArrayBuffer<TypeName> {
    read(from: RustBuffer): TypeName {
      return {
        origin: FfiConverterString.read(from),
      };
    }
    write(value: TypeName, into: RustBuffer): void {
      FfiConverterString.write(value.origin, into);
    }
    allocationSize(value: TypeName): number {
      return FfiConverterString.allocationSize(value.origin);
    }
  }
  return new FFIConverter();
})();

export type PhoenixdConfig = {
  url: string;
  password: string;
};

/**
 * Generated factory for {@link PhoenixdConfig} record objects.
 */
export const PhoenixdConfig = (() => {
  const defaults = () => ({});
  const create = (() => {
    return uniffiCreateRecord<PhoenixdConfig, ReturnType<typeof defaults>>(
      defaults
    );
  })();
  return Object.freeze({
    /**
     * Create a frozen instance of {@link PhoenixdConfig}, with defaults specified
     * in Rust, in the {@link lni} crate.
     */
    create,

    /**
     * Create a frozen instance of {@link PhoenixdConfig}, with defaults specified
     * in Rust, in the {@link lni} crate.
     */
    new: create,

    /**
     * Defaults specified in the {@link lni} crate.
     */
    defaults: () => Object.freeze(defaults()) as Partial<PhoenixdConfig>,
  });
})();

const FfiConverterTypePhoenixdConfig = (() => {
  type TypeName = PhoenixdConfig;
  class FFIConverter extends AbstractFfiConverterArrayBuffer<TypeName> {
    read(from: RustBuffer): TypeName {
      return {
        url: FfiConverterString.read(from),
        password: FfiConverterString.read(from),
      };
    }
    write(value: TypeName, into: RustBuffer): void {
      FfiConverterString.write(value.url, into);
      FfiConverterString.write(value.password, into);
    }
    allocationSize(value: TypeName): number {
      return (
        FfiConverterString.allocationSize(value.url) +
        FfiConverterString.allocationSize(value.password)
      );
    }
  }
  return new FFIConverter();
})();

const stringToArrayBuffer = (s: string): ArrayBuffer =>
  rustCall((status) =>
    nativeModule().uniffi_internal_fn_func_ffi__string_to_arraybuffer(s, status)
  );

const arrayBufferToString = (ab: ArrayBuffer): string =>
  rustCall((status) =>
    nativeModule().uniffi_internal_fn_func_ffi__arraybuffer_to_string(
      ab,
      status
    )
  );

const stringByteLength = (s: string): number =>
  rustCall((status) =>
    nativeModule().uniffi_internal_fn_func_ffi__string_to_byte_length(s, status)
  );

const FfiConverterString = (() => {
  const lengthConverter = FfiConverterInt32;
  type TypeName = string;
  class FFIConverter implements FfiConverter<ArrayBuffer, TypeName> {
    lift(value: ArrayBuffer): TypeName {
      return arrayBufferToString(value);
    }
    lower(value: TypeName): ArrayBuffer {
      return stringToArrayBuffer(value);
    }
    read(from: RustBuffer): TypeName {
      const length = lengthConverter.read(from);
      const bytes = from.readBytes(length);
      return arrayBufferToString(bytes);
    }
    write(value: TypeName, into: RustBuffer): void {
      const buffer = stringToArrayBuffer(value);
      const numBytes = buffer.byteLength;
      lengthConverter.write(numBytes, into);
      into.writeBytes(buffer);
    }
    allocationSize(value: TypeName): number {
      return lengthConverter.allocationSize(0) + stringByteLength(value);
    }
  }

  return new FFIConverter();
})();

// Error type: ApiError

// Enum: ApiError
export enum ApiError_Tags {
  Http = 'Http',
  Api = 'Api',
  Json = 'Json',
}
export const ApiError = (() => {
  type Http__interface = {
    tag: ApiError_Tags.Http;
    inner: Readonly<{ reason: string }>;
  };

  class Http_ extends UniffiError implements Http__interface {
    /**
     * @private
     * This field is private and should not be used, use `tag` instead.
     */
    readonly [uniffiTypeNameSymbol] = 'ApiError';
    readonly tag = ApiError_Tags.Http;
    readonly inner: Readonly<{ reason: string }>;
    constructor(inner: { reason: string }) {
      super('ApiError', 'Http');
      this.inner = Object.freeze(inner);
    }

    static new(inner: { reason: string }): Http_ {
      return new Http_(inner);
    }

    static instanceOf(obj: any): obj is Http_ {
      return obj.tag === ApiError_Tags.Http;
    }

    static hasInner(obj: any): obj is Http_ {
      return Http_.instanceOf(obj);
    }

    static getInner(obj: Http_): Readonly<{ reason: string }> {
      return obj.inner;
    }
  }

  type Api__interface = {
    tag: ApiError_Tags.Api;
    inner: Readonly<{ reason: string }>;
  };

  class Api_ extends UniffiError implements Api__interface {
    /**
     * @private
     * This field is private and should not be used, use `tag` instead.
     */
    readonly [uniffiTypeNameSymbol] = 'ApiError';
    readonly tag = ApiError_Tags.Api;
    readonly inner: Readonly<{ reason: string }>;
    constructor(inner: { reason: string }) {
      super('ApiError', 'Api');
      this.inner = Object.freeze(inner);
    }

    static new(inner: { reason: string }): Api_ {
      return new Api_(inner);
    }

    static instanceOf(obj: any): obj is Api_ {
      return obj.tag === ApiError_Tags.Api;
    }

    static hasInner(obj: any): obj is Api_ {
      return Api_.instanceOf(obj);
    }

    static getInner(obj: Api_): Readonly<{ reason: string }> {
      return obj.inner;
    }
  }

  type Json__interface = {
    tag: ApiError_Tags.Json;
    inner: Readonly<{ reason: string }>;
  };

  class Json_ extends UniffiError implements Json__interface {
    /**
     * @private
     * This field is private and should not be used, use `tag` instead.
     */
    readonly [uniffiTypeNameSymbol] = 'ApiError';
    readonly tag = ApiError_Tags.Json;
    readonly inner: Readonly<{ reason: string }>;
    constructor(inner: { reason: string }) {
      super('ApiError', 'Json');
      this.inner = Object.freeze(inner);
    }

    static new(inner: { reason: string }): Json_ {
      return new Json_(inner);
    }

    static instanceOf(obj: any): obj is Json_ {
      return obj.tag === ApiError_Tags.Json;
    }

    static hasInner(obj: any): obj is Json_ {
      return Json_.instanceOf(obj);
    }

    static getInner(obj: Json_): Readonly<{ reason: string }> {
      return obj.inner;
    }
  }

  function instanceOf(obj: any): obj is ApiError {
    return obj[uniffiTypeNameSymbol] === 'ApiError';
  }

  return Object.freeze({
    instanceOf,
    Http: Http_,
    Api: Api_,
    Json: Json_,
  });
})();

export type ApiError = InstanceType<
  (typeof ApiError)[keyof Omit<typeof ApiError, 'instanceOf'>]
>;

// FfiConverter for enum ApiError
const FfiConverterTypeApiError = (() => {
  const ordinalConverter = FfiConverterInt32;
  type TypeName = ApiError;
  class FFIConverter extends AbstractFfiConverterArrayBuffer<TypeName> {
    read(from: RustBuffer): TypeName {
      switch (ordinalConverter.read(from)) {
        case 1:
          return new ApiError.Http({ reason: FfiConverterString.read(from) });
        case 2:
          return new ApiError.Api({ reason: FfiConverterString.read(from) });
        case 3:
          return new ApiError.Json({ reason: FfiConverterString.read(from) });
        default:
          throw new UniffiInternalError.UnexpectedEnumCase();
      }
    }
    write(value: TypeName, into: RustBuffer): void {
      switch (value.tag) {
        case ApiError_Tags.Http: {
          ordinalConverter.write(1, into);
          const inner = value.inner;
          FfiConverterString.write(inner.reason, into);
          return;
        }
        case ApiError_Tags.Api: {
          ordinalConverter.write(2, into);
          const inner = value.inner;
          FfiConverterString.write(inner.reason, into);
          return;
        }
        case ApiError_Tags.Json: {
          ordinalConverter.write(3, into);
          const inner = value.inner;
          FfiConverterString.write(inner.reason, into);
          return;
        }
        default:
          // Throwing from here means that ApiError_Tags hasn't matched an ordinal.
          throw new UniffiInternalError.UnexpectedEnumCase();
      }
    }
    allocationSize(value: TypeName): number {
      switch (value.tag) {
        case ApiError_Tags.Http: {
          const inner = value.inner;
          let size = ordinalConverter.allocationSize(1);
          size += FfiConverterString.allocationSize(inner.reason);
          return size;
        }
        case ApiError_Tags.Api: {
          const inner = value.inner;
          let size = ordinalConverter.allocationSize(2);
          size += FfiConverterString.allocationSize(inner.reason);
          return size;
        }
        case ApiError_Tags.Json: {
          const inner = value.inner;
          let size = ordinalConverter.allocationSize(3);
          size += FfiConverterString.allocationSize(inner.reason);
          return size;
        }
        default:
          throw new UniffiInternalError.UnexpectedEnumCase();
      }
    }
  }
  return new FFIConverter();
})();

export interface FetcherInterface {
  getConfig(): string;
  getIpAddress(asyncOpts_?: { signal: AbortSignal }) /*throws*/ : Promise<Ip>;
}

export class Fetcher extends UniffiAbstractObject implements FetcherInterface {
  readonly [uniffiTypeNameSymbol] = 'Fetcher';
  readonly [destructorGuardSymbol]: UniffiRustArcPtr;
  readonly [pointerLiteralSymbol]: UnsafeMutableRawPointer;
  constructor(url: string) {
    super();
    const pointer = rustCall(
      /*caller:*/ (callStatus) => {
        return nativeModule().uniffi_lni_uniffi_fn_constructor_fetcher_new(
          FfiConverterString.lower(url),
          callStatus
        );
      },
      /*liftString:*/ FfiConverterString.lift
    );
    this[pointerLiteralSymbol] = pointer;
    this[destructorGuardSymbol] = uniffiTypeFetcherObjectFactory.bless(pointer);
  }

  public getConfig(): string {
    return FfiConverterString.lift(
      rustCall(
        /*caller:*/ (callStatus) => {
          return nativeModule().uniffi_lni_uniffi_fn_method_fetcher_get_config(
            uniffiTypeFetcherObjectFactory.clonePointer(this),
            callStatus
          );
        },
        /*liftString:*/ FfiConverterString.lift
      )
    );
  }

  public async getIpAddress(asyncOpts_?: {
    signal: AbortSignal;
  }): Promise<Ip> /*throws*/ {
    const __stack = uniffiIsDebug ? new Error().stack : undefined;
    try {
      return await uniffiRustCallAsync(
        /*rustFutureFunc:*/ () => {
          return nativeModule().uniffi_lni_uniffi_fn_method_fetcher_get_ip_address(
            uniffiTypeFetcherObjectFactory.clonePointer(this)
          );
        },
        /*pollFunc:*/ nativeModule()
          .ffi_lni_uniffi_rust_future_poll_rust_buffer,
        /*cancelFunc:*/ nativeModule()
          .ffi_lni_uniffi_rust_future_cancel_rust_buffer,
        /*completeFunc:*/ nativeModule()
          .ffi_lni_uniffi_rust_future_complete_rust_buffer,
        /*freeFunc:*/ nativeModule()
          .ffi_lni_uniffi_rust_future_free_rust_buffer,
        /*liftFunc:*/ FfiConverterTypeIp.lift.bind(FfiConverterTypeIp),
        /*liftString:*/ FfiConverterString.lift,
        /*asyncOpts:*/ asyncOpts_,
        /*errorHandler:*/ FfiConverterTypeApiError.lift.bind(
          FfiConverterTypeApiError
        )
      );
    } catch (__error: any) {
      if (uniffiIsDebug && __error instanceof Error) {
        __error.stack = __stack;
      }
      throw __error;
    }
  }

  /**
   * {@inheritDoc uniffi-bindgen-react-native#UniffiAbstractObject.uniffiDestroy}
   */
  uniffiDestroy(): void {
    if ((this as any)[destructorGuardSymbol]) {
      const pointer = uniffiTypeFetcherObjectFactory.pointer(this);
      uniffiTypeFetcherObjectFactory.freePointer(pointer);
      this[destructorGuardSymbol].markDestroyed();
      delete (this as any)[destructorGuardSymbol];
    }
  }

  static instanceOf(obj: any): obj is Fetcher {
    return uniffiTypeFetcherObjectFactory.isConcreteType(obj);
  }
}

const uniffiTypeFetcherObjectFactory: UniffiObjectFactory<FetcherInterface> = {
  create(pointer: UnsafeMutableRawPointer): FetcherInterface {
    const instance = Object.create(Fetcher.prototype);
    instance[pointerLiteralSymbol] = pointer;
    instance[destructorGuardSymbol] = this.bless(pointer);
    instance[uniffiTypeNameSymbol] = 'Fetcher';
    return instance;
  },

  bless(p: UnsafeMutableRawPointer): UniffiRustArcPtr {
    return rustCall(
      /*caller:*/ (status) =>
        nativeModule().uniffi_internal_fn_method_fetcher_ffi__bless_pointer(
          p,
          status
        ),
      /*liftString:*/ FfiConverterString.lift
    );
  },

  pointer(obj: FetcherInterface): UnsafeMutableRawPointer {
    if ((obj as any)[destructorGuardSymbol] === undefined) {
      throw new UniffiInternalError.UnexpectedNullPointer();
    }
    return (obj as any)[pointerLiteralSymbol];
  },

  clonePointer(obj: FetcherInterface): UnsafeMutableRawPointer {
    const pointer = this.pointer(obj);
    return rustCall(
      /*caller:*/ (callStatus) =>
        nativeModule().uniffi_lni_uniffi_fn_clone_fetcher(pointer, callStatus),
      /*liftString:*/ FfiConverterString.lift
    );
  },

  freePointer(pointer: UnsafeMutableRawPointer): void {
    rustCall(
      /*caller:*/ (callStatus) =>
        nativeModule().uniffi_lni_uniffi_fn_free_fetcher(pointer, callStatus),
      /*liftString:*/ FfiConverterString.lift
    );
  },

  isConcreteType(obj: any): obj is FetcherInterface {
    return (
      obj[destructorGuardSymbol] && obj[uniffiTypeNameSymbol] === 'Fetcher'
    );
  },
};
// FfiConverter for FetcherInterface
const FfiConverterTypeFetcher = new FfiConverterObject(
  uniffiTypeFetcherObjectFactory
);

export interface PhoenixdNodeInterface {
  getOffer(asyncOpts_?: { signal: AbortSignal }) /*throws*/ : Promise<string>;
}

export class PhoenixdNode
  extends UniffiAbstractObject
  implements PhoenixdNodeInterface
{
  readonly [uniffiTypeNameSymbol] = 'PhoenixdNode';
  readonly [destructorGuardSymbol]: UniffiRustArcPtr;
  readonly [pointerLiteralSymbol]: UnsafeMutableRawPointer;
  constructor(config: PhoenixdConfig) {
    super();
    const pointer = rustCall(
      /*caller:*/ (callStatus) => {
        return nativeModule().uniffi_lni_uniffi_fn_constructor_phoenixdnode_new(
          FfiConverterTypePhoenixdConfig.lower(config),
          callStatus
        );
      },
      /*liftString:*/ FfiConverterString.lift
    );
    this[pointerLiteralSymbol] = pointer;
    this[destructorGuardSymbol] =
      uniffiTypePhoenixdNodeObjectFactory.bless(pointer);
  }

  public async getOffer(asyncOpts_?: {
    signal: AbortSignal;
  }): Promise<string> /*throws*/ {
    const __stack = uniffiIsDebug ? new Error().stack : undefined;
    try {
      return await uniffiRustCallAsync(
        /*rustFutureFunc:*/ () => {
          return nativeModule().uniffi_lni_uniffi_fn_method_phoenixdnode_get_offer(
            uniffiTypePhoenixdNodeObjectFactory.clonePointer(this)
          );
        },
        /*pollFunc:*/ nativeModule()
          .ffi_lni_uniffi_rust_future_poll_rust_buffer,
        /*cancelFunc:*/ nativeModule()
          .ffi_lni_uniffi_rust_future_cancel_rust_buffer,
        /*completeFunc:*/ nativeModule()
          .ffi_lni_uniffi_rust_future_complete_rust_buffer,
        /*freeFunc:*/ nativeModule()
          .ffi_lni_uniffi_rust_future_free_rust_buffer,
        /*liftFunc:*/ FfiConverterString.lift.bind(FfiConverterString),
        /*liftString:*/ FfiConverterString.lift,
        /*asyncOpts:*/ asyncOpts_,
        /*errorHandler:*/ FfiConverterTypeApiError.lift.bind(
          FfiConverterTypeApiError
        )
      );
    } catch (__error: any) {
      if (uniffiIsDebug && __error instanceof Error) {
        __error.stack = __stack;
      }
      throw __error;
    }
  }

  /**
   * {@inheritDoc uniffi-bindgen-react-native#UniffiAbstractObject.uniffiDestroy}
   */
  uniffiDestroy(): void {
    if ((this as any)[destructorGuardSymbol]) {
      const pointer = uniffiTypePhoenixdNodeObjectFactory.pointer(this);
      uniffiTypePhoenixdNodeObjectFactory.freePointer(pointer);
      this[destructorGuardSymbol].markDestroyed();
      delete (this as any)[destructorGuardSymbol];
    }
  }

  static instanceOf(obj: any): obj is PhoenixdNode {
    return uniffiTypePhoenixdNodeObjectFactory.isConcreteType(obj);
  }
}

const uniffiTypePhoenixdNodeObjectFactory: UniffiObjectFactory<PhoenixdNodeInterface> =
  {
    create(pointer: UnsafeMutableRawPointer): PhoenixdNodeInterface {
      const instance = Object.create(PhoenixdNode.prototype);
      instance[pointerLiteralSymbol] = pointer;
      instance[destructorGuardSymbol] = this.bless(pointer);
      instance[uniffiTypeNameSymbol] = 'PhoenixdNode';
      return instance;
    },

    bless(p: UnsafeMutableRawPointer): UniffiRustArcPtr {
      return rustCall(
        /*caller:*/ (status) =>
          nativeModule().uniffi_internal_fn_method_phoenixdnode_ffi__bless_pointer(
            p,
            status
          ),
        /*liftString:*/ FfiConverterString.lift
      );
    },

    pointer(obj: PhoenixdNodeInterface): UnsafeMutableRawPointer {
      if ((obj as any)[destructorGuardSymbol] === undefined) {
        throw new UniffiInternalError.UnexpectedNullPointer();
      }
      return (obj as any)[pointerLiteralSymbol];
    },

    clonePointer(obj: PhoenixdNodeInterface): UnsafeMutableRawPointer {
      const pointer = this.pointer(obj);
      return rustCall(
        /*caller:*/ (callStatus) =>
          nativeModule().uniffi_lni_uniffi_fn_clone_phoenixdnode(
            pointer,
            callStatus
          ),
        /*liftString:*/ FfiConverterString.lift
      );
    },

    freePointer(pointer: UnsafeMutableRawPointer): void {
      rustCall(
        /*caller:*/ (callStatus) =>
          nativeModule().uniffi_lni_uniffi_fn_free_phoenixdnode(
            pointer,
            callStatus
          ),
        /*liftString:*/ FfiConverterString.lift
      );
    },

    isConcreteType(obj: any): obj is PhoenixdNodeInterface {
      return (
        obj[destructorGuardSymbol] &&
        obj[uniffiTypeNameSymbol] === 'PhoenixdNode'
      );
    },
  };
// FfiConverter for PhoenixdNodeInterface
const FfiConverterTypePhoenixdNode = new FfiConverterObject(
  uniffiTypePhoenixdNodeObjectFactory
);

/**
 * This should be called before anything else.
 *
 * It is likely that this is being done for you by the library's `index.ts`.
 *
 * It checks versions of uniffi between when the Rust scaffolding was generated
 * and when the bindings were generated.
 *
 * It also initializes the machinery to enable Rust to talk back to Javascript.
 */
function uniffiEnsureInitialized() {
  // Get the bindings contract version from our ComponentInterface
  const bindingsContractVersion = 26;
  // Get the scaffolding contract version by calling the into the dylib
  const scaffoldingContractVersion =
    nativeModule().ffi_lni_uniffi_uniffi_contract_version();
  if (bindingsContractVersion !== scaffoldingContractVersion) {
    throw new UniffiInternalError.ContractVersionMismatch(
      scaffoldingContractVersion,
      bindingsContractVersion
    );
  }
  if (
    nativeModule().uniffi_lni_uniffi_checksum_method_fetcher_get_config() !==
    25138
  ) {
    throw new UniffiInternalError.ApiChecksumMismatch(
      'uniffi_lni_uniffi_checksum_method_fetcher_get_config'
    );
  }
  if (
    nativeModule().uniffi_lni_uniffi_checksum_method_fetcher_get_ip_address() !==
    24561
  ) {
    throw new UniffiInternalError.ApiChecksumMismatch(
      'uniffi_lni_uniffi_checksum_method_fetcher_get_ip_address'
    );
  }
  if (
    nativeModule().uniffi_lni_uniffi_checksum_method_phoenixdnode_get_offer() !==
    19679
  ) {
    throw new UniffiInternalError.ApiChecksumMismatch(
      'uniffi_lni_uniffi_checksum_method_phoenixdnode_get_offer'
    );
  }
  if (
    nativeModule().uniffi_lni_uniffi_checksum_constructor_fetcher_new() !==
    47350
  ) {
    throw new UniffiInternalError.ApiChecksumMismatch(
      'uniffi_lni_uniffi_checksum_constructor_fetcher_new'
    );
  }
  if (
    nativeModule().uniffi_lni_uniffi_checksum_constructor_phoenixdnode_new() !==
    62891
  ) {
    throw new UniffiInternalError.ApiChecksumMismatch(
      'uniffi_lni_uniffi_checksum_constructor_phoenixdnode_new'
    );
  }
}

export default Object.freeze({
  initialize: uniffiEnsureInitialized,
  converters: {
    FfiConverterTypeFetcher,
    FfiConverterTypeIp,
    FfiConverterTypePhoenixdConfig,
    FfiConverterTypePhoenixdNode,
  },
});
