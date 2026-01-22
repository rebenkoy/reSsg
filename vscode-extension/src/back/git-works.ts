
import * as vscode from 'vscode';
import { simpleGit, SimpleGit, SimpleGitOptions, StatusResult } from 'simple-git';

export class GitWorks {
    static updates_branch = "updates";

    simple_api: SimpleGit | null;
    _remote_manager: any;
    constructor() {
        const base_path = vscode.workspace.workspaceFolders?.at(0)?.uri.path;
        this.simple_api = base_path === undefined ? null : simpleGit(base_path);
        this._remote_manager = null;
        this.try_change_branch();
    }

    get remote_manager() {
        if (this._remote_manager === null) {
            try {
                const ghAPI = vscode.extensions.getExtension("github.vscode-pull-request-github").exports;
                const manager = ghAPI.repositoriesManager.getManagerForFile(ghAPI.repositories[0].rootUri);
                this._remote_manager = manager.gitHubRepositories[0];
            } catch (err) {
                console.log(err);
            }
        }
        return this._remote_manager;
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

    async try_change_branch(): Promise<boolean> {
        const branches = (await this.simple_api?.branch())?.all;
        if (branches.indexOf(GitWorks.updates_branch) !== -1) {
            await this.simple_api?.checkout(GitWorks.updates_branch)
                .catch(err => {
                    console.log(err);
                    return Promise.resolve(undefined);
                });
        } else {
            await this.simple_api?.checkoutLocalBranch(GitWorks.updates_branch)
                .catch(err => {
                    console.log(err);
                    return Promise.resolve(undefined);
                });
        }
        const cur = await this.current_branch()
            .catch(err => {
                console.log(err);
                return Promise.resolve(undefined);
            });
        return cur === GitWorks.updates_branch;
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

    async push(): Promise<boolean> {
        return await this.simple_api?.push(["--set-upstream", "origin", GitWorks.updates_branch])
            .catch(err => {
                console.log(err);
                Promise.resolve(undefined);
            }) !== undefined;
    }

    async save(message: string) {
        if (await this.try_change_branch()) {
            if (await this.commit_all(message)) {
                return await this.push();
            }
        }
        return false;
    }

    async get_pr_link(): Promise<string | undefined> {
        return await this.remote_manager?.getPullRequestForBranch(GitWorks.updates_branch, this.remote_manager.remote.owner)
            .then(pr_manager => {
                if (pr_manager.isOpen) {
                    return pr_manager.html_url;
                }
                return undefined;
            });
    }
}