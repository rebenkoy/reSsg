import {Message, Pong, Ping, TellSaveStatus, RequestSaveMessage} from '../common/transport.js';

(function () {
    enum SaveButtonStyle {
        ok = "save-button--ok",
        error = "save-button--error",
        loading = "save-button--loading",
    }

    enum PRLinkStyle {
        undef = "pr-link--undef",
    }

    const debounce = <T extends (...args: any[]) => any>(
        ms: number,
        fn: T,
    ): ((...args: Parameters<T>) => void) => {

        let timeoutId: ReturnType<typeof setTimeout>;

        return function (this: any, ...args: Parameters<T>): void {
            clearTimeout(timeoutId);
            timeoutId = setTimeout(() => {
            fn.apply(this, args);
            }, ms);
        };
    };
    class State {
        running = false;
        pr_link: string | undefined = undefined;
        commit_message = "";
        constructor(updates: Partial<State> | undefined = undefined) {
            this.update(updates || {});    
        }
        update(updates: Partial<State>) {
            Object.assign(this, updates);
        }

        commit_update(redraw: boolean = true) {
            vscode.setState(this);
            if (redraw) {
                draw_state(this);
            }
        }
    }

    setInterval(() => {
        new Ping().post();
    }, 1000);
    
    const save_button = document.getElementById('save-button')! as HTMLButtonElement;
    const commit_message_input = document.getElementById('commit-message')! as HTMLTextAreaElement;
    const status_span = document.getElementById('status')! as HTMLSpanElement;
    const pr_link_a = document.getElementById('pr-link')! as HTMLAnchorElement;

    const vscode = acquireVsCodeApi();
    // vscode.setState(new State());
    const current_state: State = new State(vscode.getState() as Partial<State>);

    save_button.addEventListener('click', () => {
        save();
    });
    commit_message_input.addEventListener('keyup', (event) => {
        change_commit_message(event);
    });

    Message.setTransport((msg) => vscode.postMessage(msg));
    Pong.register_callback((msg) => setStatus(msg));
    TellSaveStatus.register_callback((msg) => tellSaveStatus(msg));
    // Handle messages sent from the extension to the webview
    window.addEventListener('message', event => {
        Message.process(event.data);
    });
    
    function tellSaveStatus(msg: TellSaveStatus) {
        clearSaveStatus();
        if (msg.ok) {
            save_button.classList.add(SaveButtonStyle.ok);
        } else {
            save_button.classList.add(SaveButtonStyle.error);
        }
        scheduleClearSaveStatus();
    }

    function clearSaveStatus() {
        save_button.classList.remove(
            SaveButtonStyle.error,
            SaveButtonStyle.loading,
            SaveButtonStyle.ok,
        );
    }
    const scheduleClearSaveStatus = debounce(1000, clearSaveStatus);

    function setStatus(msg: Pong) {
        current_state.running = msg.server_is_active;
        current_state.pr_link = msg.pull_req_url; 
        current_state.commit_update();
    }

    function change_commit_message(_: Event) {
        const commit_message = commit_message_input.value;
        current_state.update({commit_message});
        current_state.commit_update();
    }

    function check_save_params() {
        const commit_message = commit_message_input.value;
        return commit_message && commit_message.trim().length >= 10;
    }

    /** 
     * @param {string} color 
     */
    function save() {        
        save_button.classList.add(SaveButtonStyle.loading);
        scheduleClearSaveStatus();
        new RequestSaveMessage(commit_message_input.value).post();
    }

    let first_draw = true;

    const draw_state = debounce(100,
        (state: State) => {
            console.log(state);
            if (first_draw) {
                commit_message_input.value = state.commit_message;
                first_draw = false;
            }

            save_button.disabled = !check_save_params();
            
            pr_link_a.href = state.pr_link;
            if (state.pr_link === undefined) {
                pr_link_a.classList.add(PRLinkStyle.undef);
            } else {
                pr_link_a.classList.remove(PRLinkStyle.undef);
            }

            const new_status = state.running? 'running' : 'down';
            const color = state.running? 'green' : 'red';
            if (status_span === null) {
                return;
            }
            status_span.textContent = new_status;
            status_span.style['color'] = color;
        }
    );
    current_state.commit_update();
}());