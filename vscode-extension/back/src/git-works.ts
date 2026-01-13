
import * as vscode from 'vscode';
import { simpleGit, SimpleGit, SimpleGitOptions, StatusResult } from 'simple-git';

export class GitWorks {
    simple_api: SimpleGit | null;
    constructor() {
        const base_path = vscode.workspace.workspaceFolders?.at(0)?.uri.path;
        this.simple_api = base_path === undefined ? null : simpleGit(base_path);
    }

    async status(): Promise<StatusResult | undefined> {
        return await this.simple_api?.status().catch(err => Promise.resolve(undefined));
    }

    async current_branch(): Promise<string | null | undefined> {
        return await this.status().then(res => {
            if (res === undefined) {
                return undefined;
            }
            return res.current;
        });
    }

    async try_change_branch(new_branch: string): Promise<boolean> {
        let cur = await this.current_branch()
            .catch(err => {
                console.log(err);
                Promise.resolve(undefined);
            });
        if (cur === null || cur === undefined) {
            return false;
        }
        if (cur === new_branch) {
            return true;
        }
        const branches = await this.simple_api?.branch()
            .catch(err => {
                console.log(err);
                Promise.resolve(undefined);
            });
        if (branches === undefined) {
            return false;
        }
        if (new_branch in branches.all) {
            return false;
        }

        await this.simple_api?.checkoutLocalBranch(new_branch)
            .catch(err => {
                console.log(err);
                Promise.resolve(undefined);
            });

        cur = await this.current_branch()
            .catch(err => {
                console.log(err);
                Promise.resolve(undefined);
            });
        return cur === new_branch;
    }
    
    async commit_all(message: string): Promise<boolean> {
        let commit_result = undefined;
        try {
            commit_result = await this.simple_api?.add("*").commit(message, "*");
        } catch (err) {
            console.log(err);
        }
        return commit_result !== undefined;
    }

    async push(branch_name: string): Promise<boolean> {
        return await this.simple_api?.push(["--set-upstream", "origin", branch_name])
            .catch(err => {
                console.log(err);
                Promise.resolve(undefined);
            }) !== undefined;
    }

    async save(branch_name: string, message: string) {
        if (await this.try_change_branch(branch_name)) {
            if (await this.commit_all(message)) {
                return await this.push(branch_name);
            }
        }
        return false;
    }
}