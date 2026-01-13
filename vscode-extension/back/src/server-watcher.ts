import * as vscode from 'vscode';
import { ChildProcess, spawn } from 'child_process';
import { dirname } from 'path';

export class ReSsgCoontroller {
    controller: AbortController;
    process: ChildProcess | null;
    config_toml: vscode.Uri;

    constructor(config_toml: vscode.Uri, controller: AbortController) {
        this.controller = controller;
        this.process = null;
        this.config_toml = config_toml;
        this.respawn();
    }

    abort() {
        this.controller.abort();
    }
    
    respawn() {
        const { signal } = this.controller;
        if (signal.aborted) {
            return;
        }

        const process = spawn('reSsg', ['serve'], { signal, cwd: dirname(this.config_toml.fsPath) });
        process.stdout.on('data', (data) => {
            console.log(`stdout: ${data}`);
        });
        process.stderr.on('data', (data) => {
            console.log(`stderr: ${data}`);
        });
        process.on('close', (code) => {
            console.log(`reSsg exited with code ${code}, restarting`);
            this.respawn();
        });
        process.on('error', (err) => {
            if (err.name === "AbortError") {
                console.log(`reSsg was terminated by signal`);
            } else {
                console.log(`reSsg failed, terminating watcher`);
            }
            this.process = null;
        });
        this.process = process;
    }

    is_running(): boolean {
        if (this.process === null) {
            return false;
        }
        if (this.process.exitCode !== null) {
            return false;
        }
        return true;
    }
}

export function reSsg_server_watcher(config_toml: vscode.Uri): ReSsgCoontroller {
    const controller = new AbortController();
    const reSsg_controller = new ReSsgCoontroller(config_toml, controller);
    process.on('exit', (_) => {
        reSsg_controller.abort();
    });
    return reSsg_controller;
}