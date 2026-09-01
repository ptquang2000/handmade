vim.keymap.set("n", "<leader>m", function()
  vim.fn.mkdir("build", "p")
  vim.opt_local.makeprg = "sh build.sh"

  local start = vim.uv.now()
  vim.cmd("silent make!")

  local elapsed = (vim.uv.now() - start) / 1000
  local qf = vim.fn.getqflist()
  table.insert(qf, { text = string.format("Build completed in %.3fs", elapsed), lnum = 0, col = 0 })
  vim.fn.setqflist(qf, "r")

  vim.cmd("copen")
end, { desc = "Build handmade" })
