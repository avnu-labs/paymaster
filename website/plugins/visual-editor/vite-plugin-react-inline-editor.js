import path from 'path';
import { fileURLToPath } from 'url';
import { parse } from '@babel/parser';
import traverseBabel from '@babel/traverse';
import generate from '@babel/generator';
import * as t from '@babel/types';
import fs from 'fs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const VITE_PROJECT_ROOT = path.resolve(__dirname, '../..');
const EDITABLE_HTML_TAGS = ["a", "Button", "button", "p", "span", "h1", "h2", "h3", "h4", "h5", "h6", "label", "Label"];

function parseEditId(editId) {
  const parts = editId.split(':');

  if (parts.length < 3) {
    return null;
  }

  const column = parseInt(parts.at(-1), 10);
  const line = parseInt(parts.at(-2), 10);
  const filePath = parts.slice(0, -2).join(':');

  if (!filePath || isNaN(line) || isNaN(column)) {
    return null;
  }

  return { filePath, line, column };
}

function checkTagNameEditable(openingElementNode, editableTagsList) {
    if (!openingElementNode || !openingElementNode.name) return false;
    const nameNode = openingElementNode.name;

    // Check 1: Direct name (for <p>, <Button>)
    if (nameNode.type === 'JSXIdentifier' && editableTagsList.includes(nameNode.name)) {
        return true;
    }

    // Check 2: Property name of a member expression (for <motion.h1>, check if "h1" is in editableTagsList)
    if (nameNode.type === 'JSXMemberExpression' && nameNode.property && nameNode.property.type === 'JSXIdentifier' && editableTagsList.includes(nameNode.property.name)) {
        return true;
    }

    return false;
}

export default function inlineEditPlugin() {
  return {
    name: 'vite-inline-edit-plugin',
    enforce: 'pre',

    transform(code, id) {
      if (!/\.(jsx|tsx)$/.test(id) || !id.startsWith(VITE_PROJECT_ROOT) || id.includes('node_modules')) {
        return null;
      }

      const relativeFilePath = path.relative(VITE_PROJECT_ROOT, id);
      const webRelativeFilePath = relativeFilePath.split(path.sep).join('/');

      try {
        const babelAst = parse(code, {
          sourceType: 'module',
          plugins: ['jsx', 'typescript'],
          errorRecovery: true
        });

        let attributesAdded = 0;

        traverseBabel.default(babelAst, {
          enter(path) {
            if (path.isJSXOpeningElement()) {
              const openingNode = path.node;
              const elementNode = path.parentPath.node; // The JSXElement itself

              if (!openingNode.loc) {
                return;
              }

              const alreadyHasId = openingNode.attributes.some(
                (attr) => t.isJSXAttribute(attr) && attr.name.name === 'data-edit-id'
              );

              if (alreadyHasId) {
                return;
              }

              // Condition 1: Is the current element tag type editable?
              const isCurrentElementEditable = checkTagNameEditable(openingNode, EDITABLE_HTML_TAGS);
              if (!isCurrentElementEditable) {
                return;
              }

              let shouldBeDisabledDueToChildren = false;

              // Condition 2: Does the element have dynamic or editable children
              if (t.isJSXElement(elementNode) && elementNode.children) {
                // Check if element has {...props} spread attribute - disable editing if it does
                const hasPropsSpread = openingNode.attributes.some(attr => t.isJSXSpreadAttribute(attr)
                && attr.argument
                && t.isIdentifier(attr.argument)
                && attr.argument.name === 'props'
                );

                const hasDynamicChild = elementNode.children.some(child =>
                  t.isJSXExpressionContainer(child)
                );

                if (hasDynamicChild || hasPropsSpread) {
                  shouldBeDisabledDueToChildren = true;
                }
              }

              if (!shouldBeDisabledDueToChildren && t.isJSXElement(elementNode) && elementNode.children) {
                const hasEditableJsxChild = elementNode.children.some(child => {
                  if (t.isJSXElement(child)) {
                    return checkTagNameEditable(child.openingElement, EDITABLE_HTML_TAGS);
                  }

                  return false;
                });

                if (hasEditableJsxChild) {
                  shouldBeDisabledDueToChildren = true;
                }
              }

              if (shouldBeDisabledDueToChildren) {
                const disabledAttribute = t.jsxAttribute(
                  t.jsxIdentifier('data-edit-disabled'),
                  t.stringLiteral('true')
                );

                openingNode.attributes.push(disabledAttribute);
                attributesAdded++;
                return;
              }

              // Condition 3: Parent is non-editable if AT LEAST ONE child JSXElement is a non-editable type.
              if (t.isJSXElement(elementNode) && elementNode.children && elementNode.children.length > 0) {
                  let hasNonEditableJsxChild = false;
                  for (const child of elementNode.children) {
                      if (t.isJSXElement(child)) {
                          if (!checkTagNameEditable(child.openingElement, EDITABLE_HTML_TAGS)) {
                              hasNonEditableJsxChild = true;
                              break;
                          }
                      }
                  }
                  if (hasNonEditableJsxChild) {
                      const disabledAttribute = t.jsxAttribute(
                        t.jsxIdentifier('data-edit-disabled'),
                        t.stringLiteral("true")
                      );
                      openingNode.attributes.push(disabledAttribute);
                      attributesAdded++;
                      return;
                  }
              }

              // Condition 4: Is any ancestor JSXElement also editable?
              let currentAncestorCandidatePath = path.parentPath.parentPath;
              while (currentAncestorCandidatePath) {
                  const ancestorJsxElementPath = currentAncestorCandidatePath.isJSXElement()
                      ? currentAncestorCandidatePath
                      : currentAncestorCandidatePath.findParent(p => p.isJSXElement());

                  if (!ancestorJsxElementPath) {
                      break;
                  }

                  if (checkTagNameEditable(ancestorJsxElementPath.node.openingElement, EDITABLE_HTML_TAGS)) {
                      return;
                  }
                  currentAncestorCandidatePath = ancestorJsxElementPath.parentPath;
              }

              const line = openingNode.loc.start.line;
              const column = openingNode.loc.start.column + 1;
              const editId = `${webRelativeFilePath}:${line}:${column}`;

              const idAttribute = t.jsxAttribute(
                t.jsxIdentifier('data-edit-id'),
                t.stringLiteral(editId)
              );

              openingNode.attributes.push(idAttribute);
              attributesAdded++;
            }
          }
        });

        if (attributesAdded > 0) {
          const generateFunction = generate.default || generate;
          const output = generateFunction(babelAst, {
            sourceMaps: true,
            sourceFileName: webRelativeFilePath
          }, code);

          return { code: output.code, map: output.map };
        }

        return null;
      } catch (error) {
        console.error(`[vite][visual-editor] Error transforming ${id}:`, error);
        return null;
      }
    },


    // Updates source code based on the changes received from the client
    configureServer(server) {
      server.middlewares.use('/api/apply-edit', async (req, res, next) => {
        if (req.method !== 'POST') return next();

        let body = '';
        req.on('data', chunk => { body += chunk.toString(); });

        req.on('end', async () => {
          let absoluteFilePath = '';
          try {
            const { editId, newFullText } = JSON.parse(body);

            if (!editId || typeof newFullText === 'undefined') {
              res.writeHead(400, { 'Content-Type': 'application/json' });
              return res.end(JSON.stringify({ error: 'Missing editId or newFullText' }));
            }

            const parsedId = parseEditId(editId);
            if (!parsedId) {
              res.writeHead(400, { 'Content-Type': 'application/json' });
              return res.end(JSON.stringify({ error: 'Invalid editId format (filePath:line:column)' }));
            }

            const { filePath, line, column } = parsedId;

            absoluteFilePath = path.resolve(VITE_PROJECT_ROOT, filePath);
            if (filePath.includes('..') || !absoluteFilePath.startsWith(VITE_PROJECT_ROOT) || absoluteFilePath.includes('node_modules')) {
              res.writeHead(400, { 'Content-Type': 'application/json' });
              return res.end(JSON.stringify({ error: 'Invalid path' }));
            }

            const originalContent = fs.readFileSync(absoluteFilePath, 'utf-8');

            const babelAst = parse(originalContent, {
              sourceType: 'module',
              plugins: ['jsx', 'typescript'],
              errorRecovery: true
            });

            let targetNodePath = null;
            const visitor = {
              JSXOpeningElement(path) {
                const node = path.node;
                if (node.loc && node.loc.start.line === line && node.loc.start.column + 1 === column) {
                  targetNodePath = path;
                  path.stop();
                }
              }
            };
            traverseBabel.default(babelAst, visitor);

            if (!targetNodePath) {
              res.writeHead(404, { 'Content-Type': 'application/json' });
              return res.end(JSON.stringify({ error: 'Target node not found by line/column', editId }));
            }

            const generateFunction = generate.default || generate;
            const parentElementNode = targetNodePath.parentPath?.node;
            let beforeCode = '';

            if (parentElementNode && t.isJSXElement(parentElementNode)) {
              const beforeOutput = generateFunction(parentElementNode, {});
              beforeCode = beforeOutput.code;
            }

            let modified = false;

            if (parentElementNode && t.isJSXElement(parentElementNode)) {
              parentElementNode.children = [];
              if (newFullText && newFullText.trim() !== '') {
                const newTextNode = t.jsxText(newFullText);
                parentElementNode.children.push(newTextNode);
              }
              modified = true;
            }

            if (!modified) {
              res.writeHead(409, { 'Content-Type': 'application/json' });
              return res.end(JSON.stringify({ error: 'Could not apply changes to AST.' }));
            }

            let afterCode = '';
            if (parentElementNode && t.isJSXElement(parentElementNode)) {
              const afterOutput = generateFunction(parentElementNode, {});
              afterCode = afterOutput.code;
            }

            const output = generateFunction(babelAst, {});
            const newContent = output.code;

            try {
              fs.writeFileSync(absoluteFilePath, newContent, 'utf-8');
            } catch (writeError) {
              console.error(`[vite][visual-editor] Error during direct write for ${filePath}:`, writeError);
              throw writeError;
            }

            res.writeHead(200, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify({
                success: true,
                newFileContent: newContent,
                beforeCode,
                afterCode,
            }));

          } catch (error) {
            res.writeHead(500, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify({ error: 'Internal server error during edit application.' }));
          }
        });
      });
    }
  };
}