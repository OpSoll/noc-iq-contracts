/**
 * TypeScript definition generator.
 *
 * Converts a parsed contract spec (structs + functions, the same shape
 * `SLACalculatorClient.loadContractSpec()` returns) into formatted
 * TypeScript declarations, so SDK type definitions can be regenerated
 * automatically when the contract's function signatures change instead
 * of being hand-edited.
 */

export interface TypeGenField {
  name: string;
  tsType: string;
}

export interface TypeGenStruct {
  name: string;
  fields: TypeGenField[];
}

export interface TypeGenFunction {
  name: string;
  params: TypeGenField[];
  returns: string;
}

export interface TypeGenSpec {
  structs?: TypeGenStruct[];
  functions?: TypeGenFunction[];
}

/** Converts a snake_case contract function name to camelCase for TS. */
function toCamelCase(snakeCase: string): string {
  return snakeCase.replace(/_([a-z0-9])/g, (_match, char: string) =>
    char.toUpperCase(),
  );
}

/**
 * Generates formatted TypeScript interface and function declarations from
 * a contract spec. Pure string generation — writing the result to disk is
 * the caller's responsibility (see `sdk/scripts/generate-types.ts`).
 */
export function generateTypeDefinitions(spec: TypeGenSpec): string {
  const lines: string[] = [
    "// AUTO-GENERATED — do not edit by hand.",
    "// Regenerate with `npm run generate:types` from the live contract spec.",
    "",
  ];

  for (const struct of spec.structs ?? []) {
    lines.push(`export interface ${struct.name} {`);
    for (const field of struct.fields) {
      lines.push(`  ${field.name}: ${field.tsType};`);
    }
    lines.push("}", "");
  }

  for (const fn of spec.functions ?? []) {
    const params = fn.params
      .map((p) => `${p.name}: ${p.tsType}`)
      .join(", ");
    lines.push(
      `export declare function ${toCamelCase(fn.name)}(${params}): Promise<${fn.returns}>;`,
    );
  }

  return lines.join("\n") + "\n";
}
