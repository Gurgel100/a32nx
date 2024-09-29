export enum Arinc664StatusMatrix {
  NoData = 0b00,
  NoComputedData = 0b01,
  FunctionalTest = 0b10,
  NormalOperation = 0b11,
}

// Float64 not supported from JS side due to limitation of communication via SimVars.
// SInt64 only supported up to 52-bits.
export enum Arinc664ValueType {
  SInt32 = 0,
  SInt64 = 1,
  Float32 = 2,
  Boolean = 3,
  String = 4,
  Opaque = 5,
}

export interface Arinc664WordData {
  status: Arinc664StatusMatrix;

  value: number;

  type: Arinc664ValueType;

  isNoData(): boolean;

  isNoComputedData(): boolean;

  isFunctionalTest(): boolean;

  isNormalOperation(): boolean;
}

export interface Arinc664Float32Word extends Arinc664WordData {
  type: Arinc664ValueType.Float32;
}

export interface Arinc664SignedInteger32Word extends Arinc664WordData {
  type: Arinc664ValueType.SInt32;
}

export class Arinc664Word implements Arinc664WordData {
  static u64View = new Uint32Array(2);

  static f64View = new Float64Array(Arinc664Word.u64View.buffer);

  status: Arinc664StatusMatrix;

  value: number;

  type: Arinc664ValueType;

  constructor(word: number) {
    Arinc664Word.u64View[0] = (word & 0xffffffff) >>> 0;
    Arinc664Word.u64View[1] = (word >>> 32) & 0xffff;
    this.status = ((word >>> 52) & 0x3) as Arinc664StatusMatrix;
    this.type = ((word >>> 54) & 0x7) as Arinc664ValueType;
    this.value = Arinc664Word.f64View[0] | (Arinc664Word.f64View[1] << 32);
  }

  static empty(): Arinc664Word {
    return new Arinc664Word(0);
  }

  static fromSimVarValue(name: string): Arinc664Word {
    return new Arinc664Word(SimVar.GetSimVarValue(name, 'number'));
  }

  static async toSimVarValue(name: string, value: number, status: Arinc664StatusMatrix) {
    Arinc664Word.f64View[0] = value;
    const simVal = Arinc664Word.u64View[0] + Math.trunc(status) * 2 ** 32;
    return SimVar.SetSimVarValue(name, 'string', simVal.toString());
  }

  isNoData() {
    return this.status === Arinc664StatusMatrix.NoData;
  }

  isNoComputedData() {
    return this.status === Arinc664StatusMatrix.NoComputedData;
  }

  isFunctionalTest() {
    return this.status === Arinc664StatusMatrix.FunctionalTest;
  }

  isNormalOperation() {
    return this.status === Arinc664StatusMatrix.NormalOperation;
  }

  /**
   * Returns the value when normal operation, the supplied default value otherwise.
   */
  valueOr(defaultValue: number | undefined | null) {
    return this.isNormalOperation() ? this.value : defaultValue;
  }

  getBitValue(bit: number): boolean {
    return ((this.value >> (bit - 1)) & 1) !== 0;
  }

  getBitValueOr(bit: number, defaultValue: boolean | undefined | null): boolean {
    return this.isNormalOperation() ? ((this.value >> (bit - 1)) & 1) !== 0 : defaultValue;
  }

  setBitValue(bit: number, value: boolean): void {
    if (value) {
      this.value |= 1 << (bit - 1);
    } else {
      this.value &= ~(1 << (bit - 1));
    }
  }
}

export class Arinc664Register implements Arinc664WordData {
  word = 0;

  u32View = new Uint32Array(1);

  f32View = new Float32Array(this.u32View.buffer);

  status: Arinc664StatusMatrix;

  value: number;

  static empty() {
    return new Arinc664Register();
  }

  private constructor() {
    this.set(0);
  }

  set(word: number) {
    this.word = word;
    this.u32View[0] = (word & 0xffffffff) >>> 0;
    this.status = (Math.trunc(word / 2 ** 32) & 0b11) as Arinc664StatusMatrix;
    this.value = this.f32View[0];
  }

  setFromSimVar(name: string): void {
    this.set(SimVar.GetSimVarValue(name, 'number'));
  }

  isFailureWarning() {
    return this.status === Arinc664StatusMatrix.NoData;
  }

  isNoComputedData() {
    return this.status === Arinc664StatusMatrix.NoComputedData;
  }

  isFunctionalTest() {
    return this.status === Arinc664StatusMatrix.FunctionalTest;
  }

  isNormalOperation() {
    return this.status === Arinc664StatusMatrix.NormalOperation;
  }

  /**
   * Returns the value when normal operation, the supplied default value otherwise.
   */
  valueOr(defaultValue: number | undefined | null) {
    return this.isNormalOperation() ? this.value : defaultValue;
  }

  bitValue(bit: number): boolean {
    return ((this.value >> (bit - 1)) & 1) !== 0;
  }

  bitValueOr(bit: number, defaultValue: boolean | undefined | null): boolean {
    return this.isNormalOperation() ? ((this.value >> (bit - 1)) & 1) !== 0 : defaultValue;
  }
}
