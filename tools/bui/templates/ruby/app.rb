# frozen_string_literal: true

require "bui"

BUI.app do
  window "{{title}}", width: 320, height: 200 do
    count = 0
    column padding: 12, spacing: 6 do
      label "{{title}}"
      shown = label "Clicks: 0"
      button("Add one") { shown.text = "Clicks: #{count += 1}" }
    end
  end
end
