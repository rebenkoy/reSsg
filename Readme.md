# reSsg

This is an attempt to rethink a static site generator.

## !disclaimer!
This repo is a mess, it was rushed into existence in one ~20h sitting.
I obviously tried to build a site that I wanted on different frameworks, but most of them are
tailored for blogs.
Also, all the ones that I looked at expose livereload socket on a different port. 
This is a dealbreaker if you want to dev your site in a devcontainer, and I for some reason wanted to try this.

If this works out I would definitely give this project much more love and care.
Right now errors and logs are mostly handled in a way to suppress THE CRAB MONSTER <img src="https://user-images.githubusercontent.com/8974888/231858967-7c37bf1e-335b-4f5a-9760-da97be9f54bb.png" alt="Ferris with a knife" width="20px" height="12px">.

# Features
- [x] Autoreload (it is crude but it works)
- [x] minijinja templates
- [x] ability to add a collection of markdown-defined blocks
- [ ] ability to add a collection of markdown-defined pages
- [x] ability to set specific path for page, this makes organizing content easier
- [ ] helper commands
- [ ] good example
- [ ] docs
- [ ] errors
- [ ] logs
- [ ] default configs
- [ ] sass

# Small doc
Project **root** folder must contain a `config.toml` file.

In `config.toml > build` one can define **sources** dir and **output** dir (and separate static input / output).

Each directory under **sources** that contains `index.toml` file is considered a *target*

Each **target** defines its **path** and **base template**. 
Each base template will be rendered by minijinja to **{output}/{path}/index.html**

There are some custom functions in templates:
- `{{ static(path) }}` generates link to a static file with cahcebusting parameter (I use sha1 of the file).
- `{{ blocks(path, [template]) }}` renders all files in `path` in alphabetic order, each file is called a **block**.

**Block** can be either `.html` file and rendered regularly or it can be `.md` file. 
In second case the template to render is selected by optional `[template]` parameter on call or by `template` field in `toml` frontmatter (`+++`).
When the template is rendered it gets several variables:
- template: name of the template (who knows why)
- config: parsed frontmatter data (who knows how to use it, but minijinja is ok with it)
- data: dictionary. Each heading in `.md` file starts a new key, all text until next heading is considered its value.


