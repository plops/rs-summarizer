ctions.create(
    agent="antigravity-preview-05-2026",
    input=(
        "Audit https://web.dev for performance, Core Web Vitals, and SEO. "
        "Query Google's PageSpeed Insights API for both Mobile and Desktop strategies. "
        "Check search indexing with Google Search for site:web.dev. "
        "Format the output as a side-by-side scorecard table with prioritized fixes."
    ),
    environment="remote",
)

print(interaction.output_text)

The underlying Gemini model can be configured using agent_config.
Migration checklist
Note: Automate this migration with a coding agent. If you use a coding agent that supports skills (like Antigravity), install the Gemini Interactions API skill and run:

  `/gemini-interactions-api migrate my app to Gemini 3.7 Flash`

Migrate to gemini-3.7-flash

    Update Model ID: Change your target model string to gemini-3.7-flash.
    Remove deprecated sampling parameters:
        Strip temperature, top_p, and top_k from generation configs.
        Replace thinking_budget with the string enum thinking_level.
        Remove candidate_count (unsupported in Gemini 3.x).
    Enforce turn validation rules:
        Standardize multi-turn conversations on server-side previous_interaction_id.
        Remove prefilled model turns.
    Audit function calling:
        Place multimodal assets inside the response payload.
        Format inline instructions using \n\n.
        If you see Malformed_Function_Call errors tied to pre-tool text, see Workarounds for pre-tool text requirements.
        Only if using generateContent API: Ensure all FunctionResponse objects include call_id and name.
    Baseline Gemini 3.x requirements: For SDK updates and thought signature preservation, see the Gemini 3.5 Migration Checklist.

Pricing

Introductory pricing applies across Google AI Studio and Gemini Enterprise Agent Platform through December 31, 2026 for both Gemini 3.7 Flash and Gemini 3.6 Flash. From January 1, 2027, standard pricing will take effect. For details, please see pricing page.
Next steps

    Review API specs on the Models Overview.
    Explore multi-agent orchestration in the Interactions API Guide.
    Test and refine prompts in Google AI Studio.
~

	safety policy evaluation across multiple languages-0.48pp
Lower is better
Image to Text SafetyAutomated content safety evaluation measuring safety policiesNo change
Lower is better
Tone1Automated evaluation measuring objective tone of model refusal-0.47pp
Higher is better
Unjustified-refusalsAutomated evaluation measuring s ability to respond to borderline prompts while remaining safe+0.84pp
Lower is better

1 For tone and instruction following, a positive percentage increase represents an improvement in the tone of the model on sensitive topics and the s ability to follow instructions while remaining safe compared to Gemini 3 Flash. We mark improvements in green and regressions in red.

We continue to improve our internal evaluations, including refining automated evaluations to reduce false positives and negatives, as well as update query sets to ensure balance and maintain a high standard of results. The performance results reported below are computed with improved evaluations and thus are not directly comparable with performance results found in previous Gemini model cards.

We expect variation in our automated safety evaluations results, which is why we review flagged content to check for egregious or dangerous material. Our manual review confirmed losses were overwhelmingly either a) false positives or b) not egregious.
Human Red Teaming Results

We conduct manual red teaming by specialist teams who sit outside of the model development team. High-level findings are fed back to the model team. For child safety evaluations, Gemini 3.7 Flash satisfied required launch thresholds, which were developed by expert teams to protect children online and meet s commitments to child safety across our models and Google products. For content safety policies generally, including child safety, we saw similar or improved safety performance compared to Gemini 3.6 Flash. Additionally, the scope of red teaming covered potential issues outside of our strict policies, compared performance to Gemini 3.1 Pro, and found no egregious concerns.
Frontier Safety Assessment

We evaluated Gemini 3.7 Flash as outlined in our latest Frontier Safety Framework (April-2026), and found that it did not reach any tracked or critical capability levels as outlined in the table below:
	DomainKey Results for Gemini 3.7 FlashT/CCLT/CCL reached?
	CBRNWe can rule out the TCL for the CBRN domain with reasonable confidence based on the results from our testing. While Gemini 3.7 Flash demonstrates high capability in certain theoretical areas, it lacks nuanced expert knowledge and actionable depth necessary to complete priority harm journeys.

As a precautionary measure we will continue to deploy mitigations, which we assess to substantially reduce the risk posed.Uplift TCLTCL not reached
We can rule out the CCL for the CBRN domain with reasonable confidence based on the results from our testing. Expert red teaming demonstrated a modest capability uplift over web baselines and a subset of experts were able to elicit accurate and actionable information across the full harm journey for both tested scenarios, prompting us to assess that the model has reached the alert threshold for this CCL. However, due to modest average red-teaming scores, and a requirement for explicit expert steering to elicit certain details, we have assessed that Gemini 3.7 Flash falls below the CCL threshold.

As a precautionary measure we will continue to deploy mitigations, which we assess to substantially reduce the risk posed.Uplift Level 1 CCLCCL not reached
	CybersecurityGemini 3.7 Flash reaches the alert threshold for this CCL, but not the CCL itself. As a precautionary measure we will continue to deploy mitigations, which we assess to substantially reduce the risk posed.Uplift Level 1 CCLCCL not reached
Harmful ManipulationGemini 3.7 Flash demonstrates some ability to influence user beliefs and behaviors during one-on-one direct conversations in human behavioural studies. However, its overall efficacy falls beneath the CCL alert threshold. Recognizing that testing environments may under-elicit capabilities and threat actors could scale misuse absent mitigations, we continue to develop and evolve our safeguards.Level 1 CCLCCL not reached
	ML R&D and MisalignmentOn stealth evaluations, Gemini 3.7 Flash performs similarly to Gemini 3.1 Pro; on situational awareness, the model is stronger than Gemini 3.1 Pro. Gemini 3.7 Flash is observant enough to correctly assess when it is in a testing environment, but it cannot successfully bypass testing restrictions. The model does not reach the TCL.Stealth and Situational Awareness TCLTCL not reached
		Gemini 3.7 Flash can complete individual coding tasks but lacks the independence to chain them into an end-to-end research workflow without human intervention. The model does not reach the CCL alert threshold.Acceleration Level 1 CCLCCL not reached
	Automation Level 1 CCLCCL not reached

We continually work to improve the coverage and robustness of Frontier Safety safeguards. Gemini 3.7 Flash is shipping with updated safeguards to prevent misuse in the domains of Chemical, Biological, Radiological, and Nuclear (CBRN) and cyber offense.

The Gemini 3.7 Frontier Safety Framework Report will be published shortly.
Latest model cards
Gemini Robotics On-Device 2
Learn more
Gemini Robotics ER 2
Learn more
Lyria 3.5
Learn more
Gemini 3.6 Flash
Learn more
Gemini 3.5 Flash-Lite
Learn more
Gemini 3.1 Flash-Lite Image
Learn more
Follow us
Sign up for updates on our latest innovations

I accept Google's Terms and Conditions and acknowledge that my information will be used in accordance with Google's Privacy Policy.
Sign up
Build AI responsibly to benefit humanity
Models
Gemini
Gemini Omni
Nano Banana
Gemini Audio
footer_gemma__dark
Gemma
Genie
Lyria
Veo
Research
Gemini Robotics
Breakthroughs
Evals
Publications
Frontier safety
Responsibility
Science
AlphaFold
AlphaGenome
WeatherNext
AlphaEarth
AlphaEvolve
Products
Gemini app
Google AI Studio
Google Antigravity
Learn more
About
News
Careers
National Partnerships for AI
Accelerator programs
The Podcast
About Google
Google products
Privacy
Terms
Manage cookies~Googlemodelmodel
