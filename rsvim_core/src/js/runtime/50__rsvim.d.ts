//! Js runtime type declarations for `Rsvim` namespace.

interface __InternalRsvimGlobalObjectType {
  opt_get_wrap(): boolean;
  opt_set_wrap(value: boolean): void;
  opt_get_line_break(): boolean;
  opt_set_line_break(value: boolean): void;
  opt_get_break_at(): string;
  opt_set_break_at(value: string): void;
}

declare global {
  const __InternalRsvimGlobalObject: __InternalRsvimGlobalObjectType;
}

export {};