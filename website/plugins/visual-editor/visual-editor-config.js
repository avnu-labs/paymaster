export const POPUP_STYLES = `
#inline-editor-popup {
  width: 360px;
  position: fixed;
  z-index: 10000;
  background: #161718;
  color: white;
  border: 1px solid #4a5568;
  border-radius: 16px;
  padding: 8px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.2);
  flex-direction: column;
  gap: 10px;
  display: none;
}

@media (max-width: 768px) {
  #inline-editor-popup {
    width: calc(100% - 20px);
  }
}

#inline-editor-popup.is-active {
  display: flex;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
}

#inline-editor-popup.is-disabled-view {
  padding: 10px 15px;
}

#inline-editor-popup textarea {
  height: 100px;
  padding: 4px 8px;
  background: transparent;
  color: white;
  font-family: inherit;
  font-size: 0.875rem;
  line-height: 1.42;
  resize: none;
  outline: none;
}

#inline-editor-popup .button-container {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

#inline-editor-popup .popup-button {
  border: none;
  padding: 6px 16px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 0.75rem;
  font-weight: 700;
  height: 34px;
  outline: none;
}

#inline-editor-popup .save-button {
  background: #673de6;
  color: white;
}

#inline-editor-popup .cancel-button {
  background: transparent;
  border: 1px solid #3b3d4a;
  color: white;

  &:hover {
    background:#474958;
  }
}
`;

export function getPopupHTMLTemplate(saveLabel, cancelLabel) {
  return `
    <textarea></textarea>
    <div class="button-container">
      <button class="popup-button cancel-button">${cancelLabel}</button>
      <button class="popup-button save-button">${saveLabel}</button>
    </div>
  `;
};

export const EDIT_MODE_STYLES = `
  #root[data-edit-mode-enabled="true"] [data-edit-id] {
    cursor: pointer; 
    outline: 1px dashed #357DF9; 
    outline-offset: 2px;
    min-height: 1em;
  }
  #root[data-edit-mode-enabled="true"] {
    cursor: pointer;
  }
  #root[data-edit-mode-enabled="true"] [data-edit-id]:hover {
    background-color: #357DF933;
    outline-color: #357DF9; 
  }

  @keyframes fadeInTooltip {
    from {
      opacity: 0;
      transform: translateY(5px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  #inline-editor-disabled-tooltip {
    display: none; 
    opacity: 0; 
    position: absolute;
    background-color: #1D1E20;
    color: white;
    padding: 4px 8px;
    border-radius: 8px;
    z-index: 10001;
    font-size: 14px;
    border: 1px solid #3B3D4A;
    max-width: 184px;
    text-align: center;
  }

  #inline-editor-disabled-tooltip.tooltip-active {
    display: block;
    animation: fadeInTooltip 0.2s ease-out forwards;
  }
`;