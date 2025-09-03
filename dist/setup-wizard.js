// Setup Wizard JavaScript
// What: Handles the step-by-step setup process for new users
// Why: Provides guided onboarding experience
// Used by: setup-wizard.html

class SetupWizard {
    constructor() {
        this.currentStep = 0;
        this.totalSteps = 5;
        this.targetData = {
            name: '',
            code: '',
            hotkey: ''
        };
        
        this.init();
    }
    
    async init() {
        console.log('Initializing Setup Wizard...');
        
        // Wait for Tauri API
        waitForTauri(() => {
            console.log('Tauri API ready for setup wizard');
            this.setupPlatformDefaults();
        });
        
        // Set up event listeners
        this.setupEventListeners();
    }
    
    setupPlatformDefaults() {
        // Set platform-appropriate default hotkey
        const platform = navigator.platform.toLowerCase();
        let defaultHotkey = 'Cmd+Shift+J'; // macOS default
        
        if (platform.includes('win')) {
            defaultHotkey = 'Ctrl+Shift+J';
        } else if (platform.includes('linux')) {
            defaultHotkey = 'Ctrl+Shift+J';
        }
        
        this.targetData.hotkey = defaultHotkey;
        document.getElementById('hotkey-display').textContent = defaultHotkey;
    }
    
    setupEventListeners() {
        // Enter key to advance steps
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && this.currentStep < this.totalSteps - 1) {
                this.nextStep();
            }
        });
        
        // Target name input
        const targetNameInput = document.getElementById('target-name');
        if (targetNameInput) {
            targetNameInput.addEventListener('input', (e) => {
                this.targetData.name = e.target.value.trim();
                this.updateTargetNameDisplays();
            });
        }
    }
    
    updateTargetNameDisplays() {
        const displays = [
            'target-name-display',
            'target-name-display-2', 
            'final-target-name'
        ];
        
        displays.forEach(id => {
            const element = document.getElementById(id);
            if (element) {
                element.textContent = this.targetData.name || 'your contact';
            }
        });
    }
    
    async nextStep() {
        // Validate current step
        if (!this.validateCurrentStep()) {
            return;
        }
        
        // Handle step-specific actions
        await this.handleStepAction();
        
        // Move to next step
        if (this.currentStep < this.totalSteps - 1) {
            this.currentStep++;
            this.updateUI();
        }
    }
    
    previousStep() {
        if (this.currentStep > 0) {
            this.currentStep--;
            this.updateUI();
        }
    }
    
    validateCurrentStep() {
        switch (this.currentStep) {
            case 1: // Target name step
                const name = document.getElementById('target-name').value.trim();
                if (!name) {
                    this.showError('Please enter a name for your target');
                    return false;
                }
                this.targetData.name = name;
                return true;
            default:
                return true;
        }
    }
    
    async handleStepAction() {
        switch (this.currentStep) {
            case 1: // Generate code after name is entered
                await this.generateCode();
                break;
            case 4: // Save target on final step
                await this.saveTarget();
                break;
        }
    }
    
    async generateCode() {
        try {
            if (window.__TAURI__ && window.__TAURI__.invoke) {
                this.targetData.code = await window.__TAURI__.invoke('generate_code');
            } else {
                // Fallback for development
                this.targetData.code = 'demo-' + Math.random().toString(36).substr(2, 9);
            }
            
            document.getElementById('generated-code').textContent = this.targetData.code;
            document.getElementById('final-code').textContent = this.targetData.code;
            
            console.log('Generated code:', this.targetData.code);
        } catch (error) {
            console.error('Failed to generate code:', error);
            this.showError('Failed to generate code. Please try again.');
        }
    }
    
    async saveTarget() {
        try {
            if (!window.__TAURI__ || !window.__TAURI__.invoke) {
                console.log('Development mode - target would be saved:', this.targetData);
                return;
            }
            
            // First load current settings
            const settings = await window.__TAURI__.invoke('get_settings');
            
            // Create new target
            const newTarget = {
                id: Date.now().toString(36) + Math.random().toString(36).substr(2),
                label: this.targetData.name,
                code: this.targetData.code,
                target_type: 'person',
                is_primary: true, // First target is always primary
                call_defaults: {
                    start_with_audio: true,
                    start_with_video: true
                },
                created_at: new Date().toISOString(),
                notes: null
            };
            
            // Add target to settings
            settings.targets.push(newTarget);
            
            // Update hotkey
            settings.keybinds.join_primary = this.targetData.hotkey;
            
            // Save settings
            await window.__TAURI__.invoke('save_settings', { settings });
            
            console.log('Target saved successfully:', newTarget);
        } catch (error) {
            console.error('Failed to save target:', error);
            this.showError('Failed to save target. Please try again.');
        }
    }
    
    updateUI() {
        // Update step indicators
        document.querySelectorAll('.step-dot').forEach((dot, index) => {
            dot.classList.remove('active', 'completed');
            if (index < this.currentStep) {
                dot.classList.add('completed');
            } else if (index === this.currentStep) {
                dot.classList.add('active');
            }
        });
        
        // Show/hide steps
        document.querySelectorAll('.wizard-step').forEach((step, index) => {
            step.classList.toggle('active', index === this.currentStep);
        });
        
        // Update buttons
        const backBtn = document.getElementById('back-btn');
        const nextBtn = document.getElementById('next-btn');
        const finishBtn = document.getElementById('finish-btn');
        
        backBtn.style.display = this.currentStep > 0 ? 'block' : 'none';
        
        if (this.currentStep === this.totalSteps - 1) {
            nextBtn.style.display = 'none';
            finishBtn.style.display = 'block';
        } else {
            nextBtn.style.display = 'block';
            finishBtn.style.display = 'none';
            
            // Update next button text
            const buttonTexts = ['Get Started', 'Continue', 'Continue', 'Continue', 'Finish'];
            nextBtn.textContent = buttonTexts[this.currentStep] || 'Continue';
        }
        
        // Update final summary
        if (this.currentStep === this.totalSteps - 1) {
            document.getElementById('final-target-name').textContent = this.targetData.name;
            document.getElementById('final-code').textContent = this.targetData.code;
            document.getElementById('final-hotkey').textContent = this.targetData.hotkey;
        }
    }
    
    async finishSetup() {
        try {
            // Close wizard window and open main settings
            if (window.__TAURI__ && window.__TAURI__.window) {
                const { getCurrent } = window.__TAURI__.window;
                const currentWindow = getCurrent();
                await currentWindow.close();
            }
            
            console.log('Setup wizard completed successfully!');
        } catch (error) {
            console.error('Error finishing setup:', error);
            // Fallback - just close the window
            window.close();
        }
    }
    
    showError(message) {
        // Simple error display - could be enhanced with better UI
        alert(message);
    }
}

// Utility functions
async function copyCode() {
    const codeElement = document.getElementById('generated-code');
    const code = codeElement.textContent;
    
    try {
        await navigator.clipboard.writeText(code);
        
        // Visual feedback
        const originalText = codeElement.textContent;
        codeElement.textContent = 'Copied!';
        codeElement.style.background = 'var(--success)';
        codeElement.style.color = 'white';
        
        setTimeout(() => {
            codeElement.textContent = originalText;
            codeElement.style.background = 'var(--bg-primary)';
            codeElement.style.color = 'var(--accent)';
        }, 1500);
        
    } catch (error) {
        console.error('Failed to copy code:', error);
        // Fallback - select the text
        const range = document.createRange();
        range.selectNode(codeElement);
        window.getSelection().removeAllRanges();
        window.getSelection().addRange(range);
    }
}

// Global functions for button clicks
function nextStep() {
    window.setupWizard.nextStep();
}

function previousStep() {
    window.setupWizard.previousStep();
}

function finishSetup() {
    window.setupWizard.finishSetup();
}

// Initialize wizard when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    window.setupWizard = new SetupWizard();
});
