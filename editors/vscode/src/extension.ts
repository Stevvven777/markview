/**
 * The extension: one engine per window, and the commands that drive it.
 *
 * The engine is a process, and starting it is the expensive part of a
 * preview. It is therefore created on first use and kept for the life of the
 * window, shared by every document the window previews, and ended when the
 * window ends. The extension host's own death ends it too: the engine exits
 * when its pipe closes, so a host that crashes cannot leave one behind.
 */
import * as vscode from "vscode";
import * as path from "node:path";
import { Session, type DocumentSettings } from "./sidecar.js";
import { panelReport } from "./panel.js";

let engine: Session | undefined;
let starting: Promise<Session> | undefined;
const contextKey = "markview.engineFailed";

/** The engine executable: the configured one, or the one shipped here. */
function enginePath(context: vscode.ExtensionContext): string {
	const configured = vscode.workspace
		.getConfiguration("markview")
		.get<string>("enginePath");
	if (configured && configured.trim() !== "") {
		return configured;
	}
	const platform = `${process.platform}-${process.arch}`;
	const name = process.platform === "win32" ? "markview.exe" : "markview";
	return context.asAbsolutePath(`bin/${platform}/${name}`);
}

/**
 * The window's engine, started on first use.
 *
 * A second caller while the first is starting waits for the same promise, so a
 * burst of previews cannot start two engines.
 */
async function sidecar(
	context: vscode.ExtensionContext,
): Promise<Session> {
	if (engine?.running) {
		return engine;
	}
	if (starting) {
		return starting;
	}
	starting = (async () => {
		try {
			// The engine keeps its own settings, styles, fonts and image cache
			// under the extension's storage, so nothing it writes can be
			// confused with a reader the user runs themselves.
			const session = new Session(enginePath(context), [
				"--state-dir",
				context.globalStorageUri.fsPath,
			]);
			engine = session;
			await vscode.commands.executeCommand(
				"setContext",
				contextKey,
				false,
			);
			return session;
		} catch (error) {
			await vscode.commands.executeCommand(
				"setContext",
				contextKey,
				true,
			);
			throw error;
		} finally {
			starting = undefined;
		}
	})();
	return starting;
}

/** The editor's own light or dark, which is what the preview follows. */
function themeName(): "light" | "dark" {
	const kind = vscode.window.activeColorTheme.kind;
	return kind === vscode.ColorThemeKind.Light ||
		kind === vscode.ColorThemeKind.HighContrastLight
		? "light"
		: "dark";
}

/** Sends the editor's current appearance to the engine. */
async function followTheme(
	context: vscode.ExtensionContext,
): Promise<void> {
	const session = engine?.running ? engine : undefined;
	if (session) {
		const theme = themeName();
		const answer = await session.appearance({ theme });
		const { panelAppearance } = await import("./panel.js");
		panelAppearance(theme, answer.reflow);
	}
	void context;
}

/** What an export was asked for: where to write, and which template. */
interface ExportRequest {
	/** The document, defaulting to the one in front of the reader. */
	uri?: vscode.Uri;
	/** Where to write it, defaulting to a save dialog. */
	target?: vscode.Uri;
	format?: "pdf" | "png";
	/**
	 * A template the engine has, by id. Left out, the `markview.template`
	 * setting decides; `null` asks for none at all.
	 */
	template?: string | null;
	/** A template the caller holds, as MVSS rules. */
	stylesheet?: string;
}

/**
 * The template a request names, as the engine wants to hear it.
 *
 * A plain name is one of the engine's own templates and travels as an id; a
 * path is the reader's own file, which travels as the rules it holds so it
 * never has to be installed first.
 */
async function templateFor(
	request: ExportRequest,
	document: vscode.TextDocument,
): Promise<{ template?: string; stylesheet?: string }> {
	if (request.stylesheet !== undefined) {
		return { stylesheet: request.stylesheet };
	}
	// The setting is resolved for the document being exported, the way every
	// other Markview setting is: a `[markdown]` override or a folder's own
	// value has to reach it. A request that names `null` wants no template at
	// all, which is not the same as saying nothing and taking the setting.
	const named =
		request.template !== undefined
			? request.template
			: (vscode.workspace
					.getConfiguration("markview", document)
					.get<string>("template") ?? "");
	const name = (named ?? "").trim();
	if (name === "") {
		return {};
	}
	if (name.endsWith(".mvss.toml") || name.includes("/")) {
		const base = document.isUntitled
			? vscode.workspace.getWorkspaceFolder(document.uri)?.uri.fsPath ?? vscode.workspace.workspaceFolders?.[0]?.uri.fsPath
			: path.dirname(document.uri.fsPath);
		if (!path.isAbsolute(name) && !base) throw new Error("Save the document or use an absolute template path.");
		const file = vscode.Uri.file(path.resolve(base ?? "", name));
		const bytes = await vscode.workspace.fs.readFile(file);
		return { stylesheet: new TextDecoder().decode(bytes) };
	}
	return { template: name };
}

/** Offers the templates the engine has, and the reader's own files. */
async function chooseTemplate(
	session: Session,
): Promise<{ template?: string | null; stylesheet?: string } | undefined> {
	const templates = await session.styles();
	const picked = await vscode.window.showQuickPick(
		[
			{
				label: "None",
				description: "the bundled print sheet",
				value: { template: null },
			},
			...templates
				.filter((entry) => !entry.error)
				.map((entry) => ({
					label: entry.name || entry.id,
					description: entry.installed
						? `${entry.id} · installed`
						: entry.id,
					value: { template: entry.id } as {
						template?: string | null;
						stylesheet?: string;
					},
				})),
			{
				label: "Choose a template file…",
				description: "a .mvss.toml of your own",
				value: { file: true },
			},
		],
		{ title: "Export with template" },
	);
	if (!picked) {
		return undefined;
	}
	if (!("file" in picked.value)) {
		return picked.value;
	}
	const chosen = await vscode.window.showOpenDialog({
		canSelectMany: false,
		filters: { Markview: ["mvss.toml", "toml"] },
	});
	const file = chosen?.[0];
	if (!file) {
		return undefined;
	}
	const bytes = await vscode.workspace.fs.readFile(file);
	return { stylesheet: new TextDecoder().decode(bytes) };
}

/**
 * Writes the document out, in the format and with the template asked for.
 *
 * The engine owns the export; this asks for one and then shows the reader what
 * was written rather than a path they have to go and find.
 */
async function exportDocument(
	context: vscode.ExtensionContext,
	request: ExportRequest,
): Promise<void> {
	const document =
		(request.uri && (await vscode.workspace.openTextDocument(request.uri))) ||
		vscode.window.activeTextEditor?.document;
	if (!document || document.languageId !== "markdown") {
		void vscode.window.showInformationMessage(
			"Open a Markdown document first.",
		);
		return;
	}
	const format = request.format ?? "pdf";
	const session = await sidecar(context);
	// A caller that names the file — a keybinding with an argument, or a host
	// driving the command — is not asked again where to put it.
	const suffix = format === "png" ? ".png" : ".pdf";
	const target =
		request.target ??
		(await vscode.window.showSaveDialog({
			filters: format === "png" ? { PNG: ["png"] } : { PDF: ["pdf"] },
			defaultUri: document.uri.with({
				path: document.uri.path.replace(/\.(md|markdown)$/i, suffix),
			}),
		}));
	if (!target) {
		return;
	}
	const rules = await templateFor(request, document);
	try {
		const { previewDocument, panelExported } = await import("./panel.js");
		const id = await previewDocument(session, document);
		const exported = await session.export(id, target.fsPath, {
			format,
			...rules,
		});
		panelExported(target.fsPath);
		try {
			await vscode.commands.executeCommand("revealFileInOS", target);
		} catch {
			// Nothing to reveal it in.
		}
		void vscode.window.showInformationMessage(
			`Exported ${target.fsPath} (${Math.round(exported.bytes / 1024)} KB)`,
		);
	} catch (error) {
		void vscode.window.showErrorMessage(
			`Export failed: ${error instanceof Error ? error.message : String(error)}`,
		);
	}
}

export function activate(context: vscode.ExtensionContext): unknown {
	context.subscriptions.push(
		vscode.commands.registerCommand(
			"markview.openPreview",
			async (uri?: vscode.Uri) => {
				// The editor title hands over the resource it was clicked on;
				// the palette hands over nothing, and the document in front of
				// the reader is what is meant then.
				const document = uri
					? await vscode.workspace.openTextDocument(uri)
					: vscode.window.activeTextEditor?.document;
				if (!document) {
					void vscode.window.showInformationMessage(
						"Open a Markdown document first.",
					);
					return;
				}
				const session = await sidecar(context);
				await followTheme(context);
				// The panel is what draws; it is added in `panel.ts`.
				const { openPreview } = await import("./panel.js");
				await openPreview(context, session, document);
			},
		),
		vscode.commands.registerCommand("markview.copySelection", async () => {
            const { copyPreviewSelection } = await import("./panel.js");
            await copyPreviewSelection();
        }),
        vscode.commands.registerCommand("markview.closePreview", async () => {
			const { closePreview } = await import("./panel.js");
			closePreview();
		}),
		vscode.commands.registerCommand(
			"markview.exportPdf",
			async (asked?: vscode.Uri) => {
				await exportDocument(context, { target: asked, format: "pdf" });
			},
		),
		vscode.commands.registerCommand(
			"markview.exportPng",
			async (asked?: vscode.Uri) => {
				await exportDocument(context, { target: asked, format: "png" });
			},
		),
		vscode.commands.registerCommand("markview.exportWithTemplate", async () => {
			const session = await sidecar(context);
			const chosen = await chooseTemplate(session);
			if (chosen === undefined) {
				return;
			}
			const format = await vscode.window.showQuickPick(
				[
					{ label: "PDF", description: "paper, with page numbers" },
					{ label: "PNG", description: "one image of the whole document" },
				],
				{ title: "Export as" },
			);
			if (!format) {
				return;
			}
			await exportDocument(context, {
				format: format.label === "PNG" ? "png" : "pdf",
				...chosen,
			});
		}),
		vscode.commands.registerCommand("markview.revealSource", async () => {
			const { revealSelection } = await import("./panel.js");
			await revealSelection();
		}),
		vscode.window.onDidChangeActiveColorTheme(async () => {
			await followTheme(context);
		}),
	);
	// What the panel has observed, so a test can assert against it rather than
	// against a second account of it. `scrollPreviewTo` is the same entry
	// point the reveal command uses to move the view.
	return {
		panelReport,
		engineTraffic: () => ({ ...engine?.traffic }),
		exportDocument: async (request: ExportRequest) => {
			await exportDocument(context, request);
		},
		templates: async () => {
			const session = await sidecar(context);
			return session.styles();
		},
		scrollPreviewTo: async (offset: number) => {
			const { scrollPreviewTo } = await import("./panel.js");
			await scrollPreviewTo(offset);
		},
		clearPreviewSelection: async () => {
			const { clearPreviewSelection } = await import("./panel.js");
			await clearPreviewSelection();
		},
		clickPreviewAt: async (x: number, y: number) => {
			const { clickPreviewAt } = await import("./panel.js");
			await clickPreviewAt(x, y);
		},
		dragPreviewAt: async (
			from: { x: number; y: number },
			to: { x: number; y: number },
		) => {
			const { dragPreviewAt } = await import("./panel.js");
			await dragPreviewAt(from, to);
		},
		copyPreviewSelection: async () => {
			const { copyPreviewSelection } = await import("./panel.js");
			await copyPreviewSelection();
		},
		findInPreview: async (needle: string, previous = false) => {
			const { findInPreview } = await import("./panel.js");
			await findInPreview(needle, previous);
		},
	};
}

export function deactivate(): void {
	// The engine's own pipe closing would end it, but a window that closes
	// cleanly can say so rather than making it wait.
	engine?.dispose();
	engine = undefined;
}
