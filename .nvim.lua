vim.keymap.set("n", "<leader>m", function()
  vim.fn.mkdir("build", "p")
  local debug = " -g -C opt-level=3"
  local output = " --out-dir build src/unix_handmade.rs"
  vim.opt_local.makeprg = "rustc" .. debug .. output

  local start = vim.uv.now()
  vim.cmd("silent make!")

  local elapsed = (vim.uv.now() - start) / 1000
  local qf = vim.fn.getqflist()
  table.insert(qf, { text = string.format("Build completed in %.3fs", elapsed), lnum = 0, col = 0 })
  vim.fn.setqflist(qf, "r")

  vim.cmd("copen")
end, { desc = "Build handmade" })
