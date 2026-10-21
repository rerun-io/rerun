// @ts-check

/**
 * A typedoc plugin that lists, on every documented type, what uses it.
 *
 * It adds a "Used By" section with a link to each function, method, property, and type whose
 * signature names the type, so a reader can go from a type to where it appears.
 */

import { Comment, CommentTag, Converter, DeclarationReflection, ReflectionKind } from "typedoc";

/** The kinds a "Used By" entry links to: the named things a reader can look up. */
const OWNER_KINDS =
  ReflectionKind.Class |
  ReflectionKind.Interface |
  ReflectionKind.TypeAlias |
  ReflectionKind.Function |
  ReflectionKind.Method |
  ReflectionKind.Constructor |
  ReflectionKind.Property |
  ReflectionKind.Accessor |
  ReflectionKind.Variable;

/** @param {import("typedoc").Application} app */
export function load(app) {
  app.converter.on(Converter.EVENT_RESOLVE_END, (context) => {
    /** @type {Map<import("typedoc").Reflection, Set<DeclarationReflection>>} */
    const used_by = new Map();

    for (const reflection of Object.values(context.project.reflections)) {
      const owner = owner_of(reflection);
      if (!owner) {
        continue;
      }
      /** @type {Set<import("typedoc").Reflection>} */
      const targets = new Set();
      collect_targets(/** @type {any} */ (reflection).type, targets);
      for (const type of /** @type {any} */ (reflection).extendedTypes ?? []) {
        collect_targets(type, targets);
      }
      for (const target of targets) {
        if (target !== owner && target !== owner.parent) {
          used_by.set(target, (used_by.get(target) ?? new Set()).add(owner));
        }
      }
    }

    for (const [target, owners] of used_by) {
      const comment = (target.comment ??= new Comment());
      const sorted = [...owners].sort((a, b) =>
        a.getFriendlyFullName().localeCompare(b.getFriendlyFullName()),
      );
      comment.blockTags.push(
        new CommentTag(
          "@usedBy",
          sorted.flatMap((owner) => [
            { kind: "text", text: "\n- " },
            { kind: "inline-tag", tag: "@link", text: owner.getFriendlyFullName(), target: owner },
          ]),
        ),
      );
    }
  });
}

/**
 * The named declaration that `reflection` is part of, such as the method a parameter belongs to.
 *
 * A member of an object literal type has no page of its own, so it counts as part of the
 * declaration that holds the literal.
 *
 * @param {import("typedoc").Reflection} reflection
 * @returns {DeclarationReflection | undefined}
 */
function owner_of(reflection) {
  /** @type {DeclarationReflection | undefined} */
  let owner;
  /** @type {import("typedoc").Reflection | undefined} */
  let current = reflection;
  while (current) {
    if (current.kindOf(ReflectionKind.TypeLiteral)) {
      owner = undefined;
    } else if (!owner && current instanceof DeclarationReflection && current.kindOf(OWNER_KINDS)) {
      owner = current;
    }
    current = current.parent;
  }
  return owner;
}

/**
 * Every documented reflection that `type` refers to, however deeply nested.
 *
 * @param {any} type
 * @param {Set<import("typedoc").Reflection>} targets
 */
function collect_targets(type, targets) {
  if (!type || typeof type !== "object") {
    return;
  }
  if (type.type === "reference" && type.reflection) {
    targets.add(type.reflection);
  }
  if (type.type === "reflection") {
    // An object literal type: what its members and signatures use is used here too.
    const declaration = type.declaration;
    for (const child of declaration.children ?? []) {
      collect_targets(child.type, targets);
    }
    for (const signature of declaration.signatures ?? []) {
      collect_targets(signature.type, targets);
      for (const parameter of signature.parameters ?? []) {
        collect_targets(parameter.type, targets);
      }
    }
    return;
  }
  for (const nested of [
    ...(type.typeArguments ?? []),
    ...(type.types ?? []),
    ...(type.elements ?? []),
    ...(type.tail ?? []).map((/** @type {any[]} */ span) => span[0]),
    type.element,
    type.elementType,
    type.target,
    type.targetType,
    type.queryType,
    type.objectType,
    type.indexType,
    type.checkType,
    type.extendsType,
    type.trueType,
    type.falseType,
    type.parameterType,
    type.templateType,
    type.nameType,
  ]) {
    collect_targets(nested, targets);
  }
}
