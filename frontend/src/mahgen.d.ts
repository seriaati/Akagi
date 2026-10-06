// mahgen ships typings (dist/types) but its package.json `exports` doesn't
// point at them, so TypeScript can't resolve them. Declare what we use.
declare module 'mahgen' {
  export class Mahgen {
    static render(seq: string, river: boolean): Promise<string>
  }
  export class MahgenElement extends HTMLElement {}
  export class ParseError extends Error {}
}
